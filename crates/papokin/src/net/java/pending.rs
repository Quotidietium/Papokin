use std::{net::SocketAddr, num::NonZero, sync::Arc};

use bytes::Bytes;
use crossbeam::atomic::AtomicCell;
use papokin_config::networking::compression::CompressionInfo;
use papokin_data::packet::CURRENT_MC_VERSION;
use papokin_protocol::{
    ClientPacket, ConnectionState, PacketDecodeError, RawPacket, ServerPacket,
    java::{
        client::config::CConfigDisconnect,
        client::login::CLoginDisconnect,
        client::play::CPlayDisconnect,
        packet_decoder::TCPNetworkDecoder,
        packet_encoder::TCPNetworkEncoder,
        server::config::{
            SAcceptCodeOfConduct, SAcknowledgeFinishConfig, SClientInformationConfig,
            SConfigCookieResponse, SConfigPong, SConfigResourcePack, SKnownPacks, SPluginMessage,
        },
    },
    packet::MultiVersionJavaPacket,
    ser::ReadingError,
};
use papokin_util::{Hand, text::TextComponent, version::JavaMinecraftVersion};
use tokio::{
    io::{BufReader, BufWriter},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::{
    entity::player::ChatMode,
    net::{
        EncryptionError, GameProfile, PacketHandlerResult, PacketRateLimiter, PlayerConfig,
        can_not_join,
    },
    server::Server,
};

use super::JavaClient;
use super::cookie::CookieStore;

const BRAND_CHANNEL_PREFIX: &str = "minecraft:brand";

/// 登录完成前连接可保持静默的最长时间。
///
/// 玩家进入游戏后，[`JavaClient::progress_player_packets`] 会持续
/// 连接通过 keep-alive 保持有效。此前没有任何机制承担这一角色，且
/// 已接受的套接字同样没有 TCP keep-alive，因此停止通信的对端
/// 不关闭的话，其描述符会在相应对象的整个生命周期内被持有
/// 服务器。该计时器针对静默而非整个握手过程：它会在
/// 每个数据包都会重置，因此缓慢但仍在推进的登录绝不会中断。
const HANDSHAKE_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub struct PendingConnection {
    pub id: u64,
    pub address: SocketAddr,
    pub server_address: String,
    pub version: AtomicCell<JavaMinecraftVersion>,
    pub connection_state: AtomicCell<ConnectionState>,
    pub close_token: CancellationToken,
    pub network_writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    pub network_reader: TCPNetworkDecoder<BufReader<OwnedReadHalf>>,
    pub gameprofile: Option<GameProfile>,
    pub config: Option<PlayerConfig>,
    pub brand: Option<String>,
    pub packet_limiter: PacketRateLimiter,
    pub verify_token: Option<[u8; 4]>,
    pub vine_challenge: Option<[u8; 16]>,
    /// 此客户端在此期间上报的 cookie 的服务端缓存
    /// 登录与配置阶段。当发生转移时迁移到 `JavaClient` 上
    /// 连接进入游戏阶段。
    pub cookies: CookieStore,
}

impl PendingConnection {
    #[must_use]
    pub fn new(
        tcp_stream: TcpStream,
        address: SocketAddr,
        id: u64,
        packet_limiter: PacketRateLimiter,
    ) -> Self {
        let (read, write) = tcp_stream.into_split();
        Self {
            id,
            address,
            server_address: String::new(),
            version: AtomicCell::new(CURRENT_MC_VERSION),
            connection_state: AtomicCell::new(ConnectionState::HandShake),
            close_token: CancellationToken::new(),
            network_writer: TCPNetworkEncoder::new(BufWriter::new(write)),
            network_reader: TCPNetworkDecoder::new(BufReader::new(read)),
            gameprofile: None,
            config: None,
            brand: None,
            packet_limiter,
            verify_token: None,
            vine_challenge: None,
            cookies: super::cookie::new_cookie_store(),
        }
    }

    pub fn close(&self) {
        self.close_token.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.close_token.is_cancelled()
    }

    pub async fn await_close_interrupt(&self) {
        self.close_token.cancelled().await;
    }

    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> Result<(), EncryptionError> {
        let crypt_key: [u8; 16] = shared_secret
            .try_into()
            .map_err(|_| EncryptionError::SharedWrongLength)?;
        self.network_reader
            .set_encryption(&crypt_key)
            .map_err(|_| EncryptionError::AlreadyEncrypted)?;
        self.network_writer
            .set_encryption(&crypt_key)
            .map_err(|_| EncryptionError::AlreadyEncrypted)?;
        Ok(())
    }

    pub fn set_compression(&mut self, compression: &CompressionInfo) {
        if compression.level > 9 {
            error!("压缩级别无效！客户端将无法读取！");
        }

        self.network_reader
            .set_compression(compression.threshold as usize);

        self.network_writer
            .set_compression((compression.threshold as usize, compression.level));
    }

    pub async fn get_packet(&mut self) -> Option<RawPacket> {
        let close_token = self.close_token.clone();
        let packet_result = tokio::select! {
            () = close_token.cancelled() => {
                debug!("正在取消待处理连接的数据包处理");
                return None;
            },
            () = tokio::time::sleep(HANDSHAKE_IDLE_TIMEOUT) => {
                debug!(
                    "客户端 {} 在完成登录前 {} 秒未发送任何数据，已将其断开",
                    self.id,
                    HANDSHAKE_IDLE_TIMEOUT.as_secs()
                );
                return None;
            },
            res = self.network_reader.get_raw_packet() => res,
        };

        match packet_result {
            Ok(packet) => Some(packet),
            Err(err) => {
                if !matches!(err, PacketDecodeError::ConnectionClosed) {
                    debug!("解码来自客户端 {} 的数据包失败：{}", self.id, err);
                    let text = format!("读取传入数据包时出错：{err}");
                    self.kick(TextComponent::text(text)).await;
                }
                None
            }
        }
    }

    pub async fn send_packet_now<P: ClientPacket>(&mut self, packet: &P) {
        let mut packet_buf = Vec::new();
        if let Err(err) =
            JavaClient::write_packet_for_version(packet, self.version.load(), &mut packet_buf)
        {
            error!("写入数据包失败：{err:?}");
            return;
        }
        let payload = Bytes::from(packet_buf);
        if let Err(err) = self.network_writer.write_packet(payload).await {
            warn!("向客户端 {} 发送数据包失败：{}", self.id, err);
        }
        let _ = self.network_writer.flush().await;
    }

    pub async fn kick(&mut self, reason: TextComponent) {
        match self.connection_state.load() {
            ConnectionState::Login => {
                self.send_packet_now(&CLoginDisconnect::new(
                    serde_json::to_string(&reason.0).unwrap_or_else(|_| String::new()),
                ))
                .await;
            }
            ConnectionState::Config => {
                self.send_packet_now(&CConfigDisconnect::new(&reason.get_text()))
                    .await;
            }
            ConnectionState::Play => {
                self.send_packet_now(&CPlayDisconnect::new(&reason)).await;
            }
            _ => {}
        }
        debug!("正在关闭 {} 的连接", self.id);
        self.close();
    }

    pub async fn handle_login_sequence(&mut self, server: &Arc<Server>) -> PacketHandlerResult {
        while let Some(packet) = self.get_packet().await {
            if !self.packet_limiter.check_packet() {
                warn!(
                    "待处理客户端 {} 超出数据包速率限制（速率：{}/秒）",
                    self.id,
                    self.packet_limiter.max_rate()
                );
                self.kick(TextComponent::text(
                    server
                        .advanced_config
                        .networking
                        .java
                        .packet_limiter
                        .kick_message
                        .clone(),
                ))
                .await;
                return PacketHandlerResult::Stop;
            }

            match self.handle_packet(server, &packet).await {
                Ok(result) => {
                    if let Some(result) = result {
                        return result;
                    }
                }
                Err(error) => {
                    let text = format!("读取传入数据包时出错：{error}");
                    debug!("读取 id 为 {} 的传入数据包失败：{}", packet.id, error);
                    self.kick(TextComponent::text(text)).await;
                }
            }
        }
        PacketHandlerResult::Stop
    }

    pub async fn handle_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadingError> {
        match self.connection_state.load() {
            ConnectionState::HandShake => self.handle_handshake_packet(server, packet).await,
            ConnectionState::Status => self.handle_status_packet(server, packet).await,
            ConnectionState::Login | ConnectionState::Transfer => {
                self.handle_login_packet(server, packet).await
            }
            ConnectionState::Config => self.handle_config_packet(server, packet).await,
            ConnectionState::Play => Ok(None),
        }
    }

    async fn handle_handshake_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadingError> {
        debug!("正在处理握手组");
        let mut payload = &packet.payload[..];
        match packet.id {
            0 => {
                let handshake = papokin_protocol::java::server::handshake::SHandShake::read(
                    &mut payload,
                    &self.version.load(),
                )?;

                // 握手钩子：取消会在任何内容之前断开客户端连接
                // 进一步处理。Java 协议事件。
                let intention = if handshake.next_state == ConnectionState::Status {
                    crate::plugin::api::events::player::player_handshake::HandshakeIntention::Status
                } else {
                    crate::plugin::api::events::player::player_handshake::HandshakeIntention::Login
                };
                let mut handshake_event =
                    crate::plugin::api::events::player::player_handshake::PlayerHandshakeEvent::new(
                        self.address.ip().to_string(),
                        handshake.server_address.to_string(),
                        handshake.protocol_version.0,
                        intention,
                    );
                server
                    .plugin_manager
                    .fire(server, &mut handshake_event)
                    .await;
                if handshake_event.cancelled {
                    self.kick(TextComponent::text("连接被拒绝")).await;
                    return Ok(Some(PacketHandlerResult::Stop));
                }

                self.handle_handshake(server, handshake).await;
                Ok(None)
            }
            _ => Err(ReadingError::Message(format!(
                "在握手状态下无法处理 id 为 {} 的数据包",
                packet.id
            ))),
        }
    }

    async fn handle_status_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadingError> {
        debug!("正在处理状态组");
        let mut payload = &packet.payload[..];
        let version = self.version.load();

        match packet.id {
            id if id == papokin_protocol::java::server::status::SStatusRequest::to_id(version) => {
                self.handle_status_request(server).await;
                Ok(None)
            }
            id if id
                == papokin_protocol::java::server::status::SStatusPingRequest::to_id(version) =>
            {
                self.handle_ping_request(
                    papokin_protocol::java::server::status::SStatusPingRequest::read(
                        &mut payload,
                        &version,
                    )?,
                )
                .await;
                Ok(None)
            }
            _ => Err(ReadingError::Message(format!(
                "在状态状态下无法处理 id 为 {} 的 Java 客户端数据包",
                packet.id
            ))),
        }
    }

    async fn handle_login_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadingError> {
        debug!("正在处理登录组");
        let mut payload = &packet.payload[..];
        let version = self.version.load();

        match packet.id {
            id if id == papokin_protocol::java::server::login::SLoginStart::to_id(version) => {
                Ok(self
                    .handle_login_start(
                        server,
                        papokin_protocol::java::server::login::SLoginStart::read(
                            &mut payload,
                            &version,
                        )?,
                    )
                    .await)
            }
            id if id
                == papokin_protocol::java::server::login::SEncryptionResponse::to_id(version) =>
            {
                Ok(self
                    .handle_encryption_response(
                        server,
                        papokin_protocol::java::server::login::SEncryptionResponse::read(
                            &mut payload,
                            &version,
                        )?,
                    )
                    .await)
            }
            id if id
                == papokin_protocol::java::server::login::SLoginPluginResponse::to_id(version) =>
            {
                Ok(self
                    .handle_plugin_response(
                        server,
                        papokin_protocol::java::server::login::SLoginPluginResponse::read(
                            &mut payload,
                            &version,
                        )?,
                    )
                    .await)
            }
            id if id
                == papokin_protocol::java::server::login::SLoginCookieResponse::to_id(version) =>
            {
                self.handle_login_cookie_response(
                    &papokin_protocol::java::server::login::SLoginCookieResponse::read(
                        &mut payload,
                        &version,
                    )?,
                );
                Ok(None)
            }
            id if id
                == papokin_protocol::java::server::login::SLoginAcknowledged::to_id(version) =>
            {
                Ok(self.handle_login_acknowledged(server).await)
            }
            _ => Err(ReadingError::Message(format!(
                "在登录状态下无法处理 id 为 {} 的数据包",
                packet.id
            ))),
        }
    }

    async fn handle_config_packet(
        &mut self,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<Option<PacketHandlerResult>, ReadingError> {
        debug!("正在处理配置组");
        let mut payload = &packet.payload[..];
        let version = self.version.load();

        match packet.id {
            id if id == SClientInformationConfig::to_id(version) => {
                self.handle_client_information_config(SClientInformationConfig::read(
                    &mut payload,
                    &version,
                )?)
                .await;
                Ok(None)
            }
            id if id == SPluginMessage::to_id(version) => {
                self.handle_plugin_message(SPluginMessage::read(&mut payload, &version)?)
                    .await;
                Ok(None)
            }
            id if id == SAcknowledgeFinishConfig::to_id(version) => {
                let Some(profile) = self.gameprofile.clone() else {
                    return Ok(Some(PacketHandlerResult::Stop));
                };
                let config = self.config.clone().unwrap_or_default();
                self.connection_state.store(ConnectionState::Play);
                if let Some(reason) = can_not_join(&profile, &self.address, server).await {
                    self.kick(reason).await;
                    Ok(Some(PacketHandlerResult::Stop))
                } else {
                    Ok(Some(PacketHandlerResult::ReadyToPlay(profile, config)))
                }
            }
            id if id == SKnownPacks::to_id(version) => {
                self.handle_known_packs(server).await;
                Ok(None)
            }
            id if id == SConfigResourcePack::to_id(version) => {
                self.handle_resource_pack_response(
                    server,
                    SConfigResourcePack::read(&mut payload, &version)?,
                )
                .await;
                Ok(None)
            }
            id if id == SConfigCookieResponse::to_id(version) => {
                self.handle_config_cookie_response(&SConfigCookieResponse::read(
                    &mut payload,
                    &version,
                )?);
                Ok(None)
            }
            id if id == SConfigPong::to_id(version) => {
                let _pong = SConfigPong::read(&mut payload, &version)?;
                Ok(None)
            }
            id if id == SAcceptCodeOfConduct::to_id(version) => {
                let _accept = SAcceptCodeOfConduct::read(&mut payload, &version)?;
                Ok(None)
            }
            _ => Err(ReadingError::Message(format!(
                "在配置状态下无法处理 id 为 {} 的数据包",
                packet.id
            ))),
        }
    }

    pub async fn handle_client_information_config(
        &mut self,
        client_information: SClientInformationConfig<'_>,
    ) {
        debug!("正在处理客户端设置");
        if client_information.view_distance <= 0 {
            self.kick(TextComponent::text("视距不能为零或负数！")).await;
            return;
        }

        if let (Ok(main_hand), Ok(chat_mode)) = (
            Hand::try_from(client_information.main_hand.0),
            ChatMode::try_from(client_information.chat_mode.0),
        ) {
            self.config = Some(PlayerConfig {
                locale: client_information.locale.to_string(),
                view_distance: NonZero::new(client_information.view_distance as u8)
                    .unwrap_or(NonZero::<u8>::MIN),
                chat_mode,
                chat_colors: client_information.chat_colors,
                skin_parts: client_information.skin_parts,
                main_hand,
                text_filtering: client_information.text_filtering,
                server_listing: client_information.server_listing,
            });
        } else {
            self.kick(TextComponent::text("无效的手或聊天类型")).await;
        }
    }

    pub async fn handle_plugin_message(&mut self, plugin_message: SPluginMessage<'_>) {
        debug!("正在处理插件消息");
        if plugin_message.channel.starts_with(BRAND_CHANNEL_PREFIX) {
            debug!("收到客户端品牌");
            match core::str::from_utf8(plugin_message.data) {
                Ok(brand) => self.brand = Some(brand.to_string()),
                Err(e) => self.kick(TextComponent::text(e.to_string())).await,
            }
        }
    }

    pub async fn handle_resource_pack_response(
        &mut self,
        server: &Server,
        packet: SConfigResourcePack,
    ) {
        let resource_config = &server.advanced_config.resource_pack.java;
        if resource_config.enabled {
            use papokin_protocol::java::server::config::ResourcePackResponseResult;
            match packet.response_result() {
                ResourcePackResponseResult::Downloaded
                | ResourcePackResponseResult::DownloadSuccess
                | ResourcePackResponseResult::Discarded
                | ResourcePackResponseResult::Unknown(_) => {
                    if self.version.load() >= JavaMinecraftVersion::V_1_20_5 {
                        self.send_known_packs(server).await;
                    } else {
                        self.handle_known_packs(server).await;
                    }
                }
                ResourcePackResponseResult::Accepted => {}
                ResourcePackResponseResult::Declined => {
                    if resource_config.force {
                        self.kick(TextComponent::text("必需的资源包已被拒绝")).await;
                    } else if self.version.load() >= JavaMinecraftVersion::V_1_20_5 {
                        self.send_known_packs(server).await;
                    } else {
                        self.handle_known_packs(server).await;
                    }
                }
                ResourcePackResponseResult::DownloadFail => {
                    if resource_config.force {
                        self.kick(TextComponent::text("下载资源包失败")).await;
                    } else if self.version.load() >= JavaMinecraftVersion::V_1_20_5 {
                        self.send_known_packs(server).await;
                    } else {
                        self.handle_known_packs(server).await;
                    }
                }
                ResourcePackResponseResult::InvalidUrl => {
                    self.kick(TextComponent::text("无效的资源包 URL")).await;
                }
                ResourcePackResponseResult::ReloadFailed => {
                    self.kick(TextComponent::text("重载资源包失败")).await;
                }
            }
        }
    }

    pub fn handle_config_cookie_response(&self, packet: &SConfigCookieResponse<'_>) {
        debug!(
            "收到 cookie_response[config]：key: \"{}\"，payload_length: \"{:?}\"",
            packet.key,
            packet.payload.as_ref().map(|p| p.len())
        );
        super::cookie::apply_cookie_response(&self.cookies, packet.key, packet.payload);
    }
}
