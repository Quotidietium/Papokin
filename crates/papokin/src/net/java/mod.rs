use papokin_protocol::java::client::play::{
    CChunkBatchEnd, CChunkBatchStart, CLightUpdate, CPlayDisconnect,
};
use papokin_world::level::SyncChunk;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{collections::VecDeque, io::Write, sync::Arc};

use bytes::Bytes;
use crossbeam::atomic::AtomicCell;
use papokin_data::translation;
use papokin_protocol::java::server::play::{
    SAttack, SBlockEntityTagQuery, SBundleItemSelected, SChangeDifficulty, SChangeGameMode,
    SChatAck, SChatCommand, SChatCommandSigned, SChatMessage, SChunkBatch, SClickSlot,
    SClientCommand, SClientInformationPlay, SClientTickEnd, SCloseContainer, SCommandSuggestion,
    SConfigurationAcknowledged, SConfirmTeleport, SContainerButtonClick,
    SContainerSlotStateChanged, SCookieResponse as SPCookieResponse, SCustomPayload,
    SDebugSampleSubscription, SDebugSubscriptionRequest, SEditBook, SEntityTagQuery, SInteract,
    SJigsawGenerate, SLockDifficulty, SMoveVehicle, SPaddleBoat, SPickItemFromBlock, SPlaceRecipe,
    SPlayPingRequest, SPlayPong, SPlayResourcePack, SPlayerAbilities, SPlayerAction,
    SPlayerCommand, SPlayerInput, SPlayerLoaded, SPlayerPosition, SPlayerPositionRotation,
    SPlayerRotation, SPlayerSession, SRecipeBookChangeSettings, SRecipeBookSeenRecipe, SRenameItem,
    SSeenAdvancement, SSelectTrade, SSetBeacon, SSetCommandBlock, SSetCommandMinecart,
    SSetCreativeSlot, SSetGameRule, SSetHeldItem, SSetJigsawBlock, SSetPlayerGround,
    SSetStructureBlock, SSetTestBlock, SSpectateEntity, SSwingArm, STeleportToEntity,
    STestInstanceBlockAction, SUpdateSign, SUseItem, SUseItemOn,
};
use papokin_protocol::packet::MultiVersionJavaPacket;
use papokin_protocol::{
    ClientPacket, ConnectionState, MAX_PACKET_SIZE, PacketDecodeError, PacketEncodeError,
    RawPacket, ServerPacket,
    codec::var_int::VarInt,
    java::{
        client::{config::CConfigDisconnect, login::CLoginDisconnect},
        packet_decoder::TCPNetworkDecoder,
        packet_encoder::TCPNetworkEncoder,
    },
    ser::{NetworkWriteExt, WritingError},
};
use papokin_util::text::TextComponent;
use papokin_util::version::JavaMinecraftVersion;
use tokio::{
    io::{BufReader, BufWriter},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::oneshot,
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender, error::TryRecvError},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, warn};

pub mod chunk_data;
pub mod config;
pub mod cookie;
pub mod handshake;
pub mod login;
pub mod pending;
pub mod play;
pub mod recipe_helper;
pub mod status;

pub use chunk_data::{CChunkData, ChunkLightExt};

use arc_swap::ArcSwap;
use cookie::CookieStore;
use pending::PendingConnection;

use crate::entity::player::Player;
use crate::net::{
    GameProfile, MAX_PENDING_BYTES, PacketHandlerResult, PacketRateLimiter, PlayerConfig,
    decrement_pending_bytes,
};
use crate::plugin::api::events::world::chunk_send::ChunkSend;
use crate::plugin::player::player_custom_payload::PlayerCustomPayloadEvent;
use crate::{error::PapokinError, server::Server};

pub struct JavaClient {
    pub id: u64,
    pub version: AtomicCell<JavaMinecraftVersion>,
    /// 客户端的游戏档案信息。直接字段（无锁）。
    pub gameprofile: GameProfile,
    /// 客户端的配置设置。无锁的 `ArcSwap`。
    pub config: ArcSwap<PlayerConfig>,
    /// 用于连接服务器的地址，在握手阶段发送。直接字段。
    pub server_address: String,
    /// 客户端当前的连接状态（例如 Handshaking、Status、Play）。
    pub connection_state: AtomicCell<ConnectionState>,
    /// 客户端的 IP 地址。直接字段（无锁）。
    pub address: SocketAddr,
    /// 客户端的品牌或整合包信息。无锁的 `ArcSwap`。
    pub brand: ArcSwap<Option<String>>,
    /// 关联的玩家引用。无锁的 `ArcSwap`。
    pub player: ArcSwap<Option<Arc<Player>>>,
    /// 与该客户端关联的任务集合。移除客户端时会等待这些任务完成。
    tasks: TaskTracker,
    rt_handle: tokio::runtime::Handle,
    /// 当此客户端关闭时触发的通知器。
    close_token: CancellationToken,
    /// 一个普通优先级队列，存放待发送到网络的已序列化数据包。
    outgoing_packet_queue_send: UnboundedSender<OutgoingPacket>,
    /// 一个普通优先级队列，存放待发送到网络的已序列化数据包。
    outgoing_packet_queue_recv: Option<UnboundedReceiver<OutgoingPacket>>,
    /// 一个高优先级队列，存放待发送到网络的已序列化数据包。
    outgoing_packet_priority_send: UnboundedSender<OutgoingPacket>,
    /// 一个高优先级队列，存放待发送到网络的已序列化数据包。
    outgoing_packet_priority_recv: Option<UnboundedReceiver<OutgoingPacket>>,
    /// 跟踪输出队列中已缓冲的负载字节总数。
    pub pending_bytes: Arc<AtomicUsize>,
    /// 用于传出数据包的数据包编码器。
    network_writer: std::sync::Mutex<Option<TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>>>,
    /// 用于传入数据包的数据包解码器。
    network_reader: std::sync::Mutex<Option<TCPNetworkDecoder<BufReader<OwnedReadHalf>>>>,
    /// Keep Alive：
    ///
    /// 发送 keep alive 数据包后是否正在等待响应。
    pub wait_for_keep_alive: AtomicBool,
    /// 当本刻收到任何移动数据包时设为 `true`。
    /// 在 `SClientTickEnd`（≥1.21.4）时，如果仍为 `false`，玩家的已知
    /// 移动量会被清零（即原地未动）。与原版 `receivedMovementThisTick` 一致。
    pub received_movement_this_tick: AtomicBool,
    /// 我们发送的保活数据包负载。客户端应以相同的 id 响应。
    pub keep_alive_id: AtomicCell<i64>,
    /// 我们上次发送保活数据包的时间。
    pub last_keep_alive_time: AtomicCell<Instant>,
    /// 上次从客户端收到任意数据包的时间。
    pub last_packet_time: AtomicCell<Instant>,
    /// 最近的在途 keep alive ID 及其发送时间戳。
    pub pending_keep_alives: std::sync::Mutex<Vec<(i64, Instant)>>,

    pub packet_sequence: AtomicI32,
    /// 用于客户端传入数据包的数据包速率限制器。
    pub packet_limiter: PacketRateLimiter,
    /// 此客户端已上报 cookie 的服务端缓存（携带
    /// 从待处理连接继承而来，因此在登录或
    /// 配置在游戏中仍然可见）。
    pub cookies: CookieStore,
    /// 已请求、等待客户端回报的 cookie 键；用于拒绝未请求的
    /// cookie 响应（防止伪造键无界增长缓存）。
    pub pending_cookie_requests: crate::net::java::cookie::PendingCookieRequests,
}

pub enum OutgoingPacketType {
    Normal,
    HighPriority,
}

struct OutgoingPacket {
    data: Bytes,
    completion: Option<oneshot::Sender<()>>,
}

const MAX_FRAME_BATCH_DATA_SIZE: usize = MAX_PACKET_SIZE as usize;

fn take_frame_batch(packets: &mut VecDeque<OutgoingPacket>) -> Vec<OutgoingPacket> {
    let mut batch = Vec::new();
    let mut data_len = 0usize;

    while let Some(packet) = packets.pop_front() {
        let next_len = data_len.saturating_add(packet.data.len());
        if !batch.is_empty() && next_len > MAX_FRAME_BATCH_DATA_SIZE {
            packets.push_front(packet);
            break;
        }

        data_len = next_len;
        batch.push(packet);
    }

    batch
}

fn frame_packet_batch(
    mut writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    batch: &[OutgoingPacket],
) -> (
    TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    Vec<u8>,
    Option<PacketEncodeError>,
) {
    let mut frame = Vec::new();
    let mut frame_err = None;
    for packet in batch {
        if let Err(err) = writer.frame_packet(&packet.data, &mut frame) {
            frame_err = Some(err);
            break;
        }
    }
    (writer, frame, frame_err)
}

async fn frame_batch_maybe_offload(
    writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    packet_batch: Vec<OutgoingPacket>,
) -> Result<
    (
        TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
        Vec<OutgoingPacket>,
        Vec<u8>,
        Option<PacketEncodeError>,
    ),
    tokio::task::JoinError,
> {
    let needs_offload = packet_batch
        .iter()
        .any(|packet| writer.is_compressing_packet(&packet.data));

    if needs_offload {
        tokio::task::spawn_blocking(move || {
            let (writer, frame, frame_err) = frame_packet_batch(writer, &packet_batch);
            (writer, packet_batch, frame, frame_err)
        })
        .await
    } else {
        let (writer, frame, frame_err) = frame_packet_batch(writer, &packet_batch);
        Ok((writer, packet_batch, frame, frame_err))
    }
}

impl OutgoingPacket {
    const fn normal(data: Bytes) -> Self {
        Self {
            data,
            completion: None,
        }
    }

    const fn high_priority(data: Bytes, completion: oneshot::Sender<()>) -> Self {
        Self {
            data,
            completion: Some(completion),
        }
    }
}

impl JavaClient {
    #[must_use]
    pub fn from_pending(
        pending: PendingConnection,
        gameprofile: GameProfile,
        config: PlayerConfig,
    ) -> Self {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        let (priority_send, priority_recv) = tokio::sync::mpsc::unbounded_channel();

        Self {
            id: pending.id,
            gameprofile,
            config: ArcSwap::from_pointee(config),
            server_address: pending.server_address,
            address: pending.address,
            connection_state: pending.connection_state,
            close_token: pending.close_token,
            tasks: TaskTracker::new(),
            rt_handle: tokio::runtime::Handle::current(),
            outgoing_packet_queue_send: send,
            outgoing_packet_queue_recv: Some(recv),
            outgoing_packet_priority_send: priority_send,
            outgoing_packet_priority_recv: Some(priority_recv),
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            version: pending.version,
            network_writer: std::sync::Mutex::new(Some(pending.network_writer)),
            network_reader: std::sync::Mutex::new(Some(pending.network_reader)),
            brand: ArcSwap::from_pointee(pending.brand),
            player: ArcSwap::from_pointee(None),
            wait_for_keep_alive: AtomicBool::new(false),
            received_movement_this_tick: AtomicBool::new(false),
            keep_alive_id: AtomicCell::new(0),
            last_keep_alive_time: AtomicCell::new(Instant::now()),
            last_packet_time: AtomicCell::new(Instant::now()),
            pending_keep_alives: std::sync::Mutex::new(Vec::new()),
            packet_sequence: AtomicI32::new(-1),
            packet_limiter: pending.packet_limiter,
            cookies: pending.cookies,
            pending_cookie_requests: pending.pending_cookie_requests,
        }
    }

    pub fn set_player(&self, player: Arc<Player>) {
        self.player.store(Arc::new(Some(player)));
    }

    pub async fn progress_player_packets(&self, player: &Arc<Player>, server: &Arc<Server>) {
        let Some(mut network_reader) = self
            .network_reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        else {
            return;
        };

        let keep_alive_time = server.advanced_config.networking.java.keep_alive_time;
        let mut keep_alive_interval =
            tokio::time::interval(std::time::Duration::from_secs(keep_alive_time.max(1)));
        let timeout_duration =
            std::time::Duration::from_secs(keep_alive_time.saturating_mul(2).max(1));

        // 跳过紧接着的第一刻，避免在他们加入的同一毫秒就发送 keep-alive
        keep_alive_interval.tick().await;

        loop {
            tokio::select! {
                // 保活定时器
                _ = keep_alive_interval.tick() => {
                    // 检查客户端是否因 keep-alive 响应超时或无数据包活动而超时
                    let has_timed_out = {
                        let pending = self
                            .pending_keep_alives
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        pending.iter().any(|(_, send_time)| send_time.elapsed() > timeout_duration)
                    } || (self.wait_for_keep_alive.load(Ordering::Relaxed) && self.last_keep_alive_time.load().elapsed() > timeout_duration)
                      || (self.last_packet_time.load().elapsed() > timeout_duration);

                    if has_timed_out {
                        self.kick(papokin_macros::translate!(translation::java::DISCONNECT_TIMEOUT)).await;
                        break;
                    }

                    let keep_alive_id = i64::from(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i32,
                    );

                    self.keep_alive_id.store(keep_alive_id);
                    self.wait_for_keep_alive.store(true, Ordering::Relaxed);
                    self.last_keep_alive_time.store(Instant::now());
                    {
                        let mut pending = self
                            .pending_keep_alives
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        pending.push((keep_alive_id, Instant::now()));
                        if pending.len() > 16 {
                            pending.remove(0);
                        }
                    }
                    let packet = papokin_protocol::java::client::play::CKeepAlive::new(keep_alive_id);
                    self.enqueue_client_packet(&packet).await;
                }

                () = self.close_token.cancelled() => {
                    break;
                }

                // 传入数据包
                packet_opt = self.get_packet_with_reader(&mut network_reader) => {
                    let Some(packet) = packet_opt else {
                        break;
                    };
                    self.last_packet_time.store(Instant::now());

                    if !self.packet_limiter.check_packet() {
                        warn!(
                            "客户端 {}（{}）超出数据包速率限制（速率：{}/秒）",
                            self.id,
                            self.gameprofile.name,
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
                        break;
                    }

                    player.inbound_packets.push(packet);
                }
            }
        }
    }

    pub async fn await_tasks(&self) {
        self.tasks.close();
        self.tasks.wait().await;
    }

    /// 生成与此客户端关联的任务。使用此方法生成的所有任务都会被等待
    /// 当客户端断开连接时。这意味着任务应在合理的时间内完成，或者选择
    /// 依赖 `Self::await_close_interrupt` 在客户端关闭时取消任务
    ///
    ///返回 `Option<JoinHandle<F::Output>>`。若客户端已关闭，则返回 `None`。
    pub fn spawn_task<F>(&self, task: F) -> Option<JoinHandle<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        if self.close_token.is_cancelled() {
            None
        } else {
            let _guard = self.rt_handle.enter();
            Some(self.tasks.spawn(task))
        }
    }

    pub async fn send_chunks(&self, chunks: &[SyncChunk]) {
        let player = self.player.load_full();
        let Some(player) = player.as_ref() else {
            return;
        };
        let Some(server) = player.world().server.upgrade() else {
            return;
        };

        let mut valid_chunks = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let mut event = ChunkSend::new(player.world(), chunk.clone());
            server.plugin_manager.fire(&server, &mut event).await;
            if !event.cancelled {
                valid_chunks.push(chunk.clone());
            }
        }

        if valid_chunks.is_empty() {
            return;
        }

        let version = self.version.load();
        let (tx, rx) = oneshot::channel();
        rayon::spawn(move || {
            let mut serialized = Vec::with_capacity(valid_chunks.len());
            for chunk in valid_chunks {
                let mut buf = Vec::with_capacity(32 * 1024);
                if let Err(err) = buf.write_var_int(&VarInt(CChunkData::to_id(version))) {
                    error!("写入区块数据 id 失败：{err:?}");
                    continue;
                }
                if let Err(err) = CChunkData(&chunk).write_packet_data(&mut buf, &version) {
                    error!("写入区块数据失败：{err:?}");
                    continue;
                }

                let light_buf = if version >= JavaMinecraftVersion::V_1_14
                    && version < JavaMinecraftVersion::V_1_18
                {
                    match CLightUpdate::from_chunk(&chunk, version) {
                        Ok(light_packet) => {
                            let mut light_buf = Vec::new();
                            if let Err(err) =
                                light_buf.write_var_int(&VarInt(CLightUpdate::to_id(version)))
                            {
                                error!("写入光照更新 id 失败：{err:?}");
                                None
                            } else if let Err(err) =
                                light_packet.write_packet_data(&mut light_buf, &version)
                            {
                                error!("写入光照更新数据失败：{err:?}");
                                None
                            } else {
                                Some(Bytes::from(light_buf))
                            }
                        }
                        Err(err) => {
                            error!("创建光照更新数据包失败：{err:?}");
                            None
                        }
                    }
                } else {
                    None
                };

                serialized.push((Bytes::from(buf), light_buf));
            }
            let _ = tx.send(serialized);
        });

        let Ok(serialized) = rx.await else {
            return;
        };
        let sent_count = serialized.len();
        if sent_count == 0 {
            return;
        }

        if version >= JavaMinecraftVersion::V_1_20_2 {
            self.send_packet(&CChunkBatchStart).await;
        }

        // 让整个批次留在优先队列中。否则批次结束可能超越区块
        // 数据排在普通通道上，导致客户端无法渲染这些区块。
        for (chunk_data, light_data) in serialized {
            self.send_packet_now_data(chunk_data).await;
            if let Some(light_data) = light_data {
                self.send_packet_now_data(light_data).await;
            }
        }

        if version >= JavaMinecraftVersion::V_1_20_2 {
            self.send_packet(&CChunkBatchEnd::new(sent_count as u16))
                .await;
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn enqueue_packet(&self, packet_data: Bytes) {
        self.try_enqueue_packet_data(packet_data);
    }

    #[allow(clippy::unused_async)]
    pub async fn enqueue_packet_data(&self, packet_data: Bytes) {
        self.try_enqueue_packet_data(packet_data);
    }

    pub fn try_enqueue_packet(&self, packet_data: Bytes) {
        self.try_enqueue_packet_data(packet_data);
    }

    pub fn try_enqueue_packet_data(&self, packet_data: Bytes) {
        if self.close_token.is_cancelled() {
            return;
        }

        let packet_len = packet_data.len();
        let prev_bytes = self.pending_bytes.fetch_add(packet_len, Ordering::AcqRel);
        let new_bytes = prev_bytes.saturating_add(packet_len);

        if new_bytes > MAX_PENDING_BYTES {
            decrement_pending_bytes(&self.pending_bytes, packet_len);
            if !self.close_token.is_cancelled() {
                warn!(
                    "客户端 {} 的出站数据包缓冲区溢出（{} 字节 > {} 字节）。正在关闭连接。",
                    self.id, new_bytes, MAX_PENDING_BYTES
                );
                self.close();
            }
            return;
        }

        if let Err(err) = self
            .outgoing_packet_queue_send
            .send(OutgoingPacket::normal(packet_data))
        {
            decrement_pending_bytes(&self.pending_bytes, packet_len);
            // 如果我们已经关闭，预期这里会失败
            if !self.close_token.is_cancelled() {
                warn!(
                    "无法将数据包加入客户端 {} 的出站数据包队列：{}",
                    self.id, err
                );
                // 现在我们需要关闭与客户端的连接，因为流处于
                // 未知状态
                self.close();
            }
        }
    }

    pub async fn await_close_interrupt(&self) {
        self.close_token.cancelled().await;
    }

    pub async fn get_packet_with_reader(
        &self,
        network_reader: &mut TCPNetworkDecoder<BufReader<OwnedReadHalf>>,
    ) -> Option<RawPacket> {
        tokio::select! {
            () = self.await_close_interrupt() => {
                debug!("正在取消玩家数据包处理");
                None
            },
            packet_result = network_reader.get_raw_packet() => {
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
        }
    }

    pub fn try_kick(&self, reason: &TextComponent) {
        let serialized = match self.connection_state.load() {
            ConnectionState::Login => {
                let packet = CLoginDisconnect::new(
                    serde_json::to_string(&reason.0).unwrap_or_else(|_| String::new()),
                );
                self.serialize_packet(&packet).ok()
            }
            ConnectionState::Config => {
                let reason_text = reason.clone().get_text();
                let packet = CConfigDisconnect::new(&reason_text);
                self.serialize_packet(&packet).ok()
            }
            ConnectionState::Play => {
                let packet = CPlayDisconnect::new(reason);
                self.serialize_packet(&packet).ok()
            }
            _ => None,
        };

        if let Some(data) = serialized {
            let packet_len = data.len();
            let _ = self.pending_bytes.fetch_add(packet_len, Ordering::AcqRel);
            let _ = self
                .outgoing_packet_priority_send
                .send(OutgoingPacket::normal(data));
        }
        let reason_text = reason.clone().get_text();
        warn!("正在关闭 {} 的连接：{reason_text}", self.id);
        self.close();
    }

    pub async fn kick(&self, reason: TextComponent) {
        self.kick_explicit(&reason, true).await;
    }

    pub async fn kick_explicit(&self, reason: &TextComponent, send_packet: bool) {
        if send_packet {
            match self.connection_state.load() {
                ConnectionState::Login => {
                    // TextComponent 实现了 Serialize 并以字节而非 String 写入，这就是我们只使用 content 的原因
                    self.send_packet(&CLoginDisconnect::new(
                        serde_json::to_string(&reason.0).unwrap_or_else(|_| String::new()),
                    ))
                    .await;
                }
                ConnectionState::Config => {
                    self.send_packet(&CConfigDisconnect::new(&reason.clone().get_text()))
                        .await;
                }
                ConnectionState::Play => self.send_packet(&CPlayDisconnect::new(reason)).await,
                _ => {}
            }
        }
        let reason_text = reason.clone().get_text();
        warn!("正在关闭 {} 的连接：{reason_text}", self.id);
        self.close();
    }

    pub async fn send_packet_now(&self, packet: Bytes) {
        self.send_packet_now_data(packet).await;
    }

    pub async fn send_packet_now_data(&self, packet: Bytes) {
        if self.close_token.is_cancelled() {
            return;
        }

        let packet_len = packet.len();
        let prev_bytes = self.pending_bytes.fetch_add(packet_len, Ordering::AcqRel);
        let new_bytes = prev_bytes.saturating_add(packet_len);

        if new_bytes > MAX_PENDING_BYTES {
            decrement_pending_bytes(&self.pending_bytes, packet_len);
            if !self.close_token.is_cancelled() {
                warn!(
                    "客户端 {} 的出站数据包缓冲区溢出（{} 字节 > {} 字节）。正在关闭连接。",
                    self.id, new_bytes, MAX_PENDING_BYTES
                );
                self.close();
            }
            return;
        }

        let (completion_tx, completion_rx) = oneshot::channel();

        if let Err(err) = self
            .outgoing_packet_priority_send
            .send(OutgoingPacket::high_priority(packet, completion_tx))
        {
            decrement_pending_bytes(&self.pending_bytes, packet_len);
            // 如果我们已关闭，数据包预计会失败
            if !self.close_token.is_cancelled() {
                warn!(
                    "无法将高优先级数据包加入客户端 {} 的出站数据包队列：{}",
                    self.id, err
                );
                // 现在我们需要关闭与客户端的连接，因为流处于
                // 未知状态
                self.close();
            }
            return;
        }

        if completion_rx.await.is_err() && !self.close_token.is_cancelled() {
            // 发送数据包的任务在确认写入之前就被丢弃了。
            self.close();
        }
    }

    pub fn write_packet_for_version<P: ClientPacket>(
        packet: &P,
        version: JavaMinecraftVersion,
        write: impl Write,
    ) -> Result<(), WritingError> {
        papokin_protocol::java::packet_encoder::write_packet(packet, &version, write)
    }

    pub fn serialize_packet_for_version<P: ClientPacket>(
        packet: &P,
        version: JavaMinecraftVersion,
    ) -> Result<Bytes, WritingError> {
        papokin_protocol::java::packet_encoder::serialize_packet(packet, &version)
    }

    pub fn serialize_packet<P: ClientPacket>(&self, packet: &P) -> Result<Bytes, WritingError> {
        Self::serialize_packet_for_version(packet, self.version.load())
    }

    pub fn try_send_packet<P: ClientPacket>(&self, packet: &P) {
        if let Ok(data) = self.serialize_packet(packet) {
            self.try_enqueue_packet(data);
        }
    }

    pub async fn send_packet<P: ClientPacket>(&self, packet: &P) {
        if let Ok(data) = self.serialize_packet(packet) {
            self.send_packet_now(data).await;
        }
    }

    pub async fn enqueue_client_packet<P: ClientPacket>(&self, packet: &P) {
        if let Ok(data) = self.serialize_packet(packet) {
            self.enqueue_packet(data).await;
        }
    }

    pub fn write_packet<P: ClientPacket>(
        &self,
        packet: &P,
        write: impl Write,
    ) -> Result<(), WritingError> {
        Self::write_packet_for_version(packet, self.version.load(), write)
    }

    /// 处理传入的数据包，根据当前连接状态将其路由到相应的处理器。
    ///
    /// 此函数接收一个 `RawPacket`，并根据当前连接状态将其路由到相应的处理器。
    /// 它支持以下连接状态：
    ///
    /// - **握手：** 处理握手数据包。
    /// - **状态：** 处理状态请求和 ping 数据包。
    /// - **登录/转移：** 处理登录和转移数据包。
    /// - **配置：** 处理配置数据包。
    #[expect(clippy::too_many_lines)]
    pub fn start_outgoing_packet_task(&mut self) {
        const MAX_BATCH_SIZE: usize = 64;

        let Some(mut packet_receiver) = self.outgoing_packet_queue_recv.take() else {
            return;
        };
        let Some(mut priority_packet_receiver) = self.outgoing_packet_priority_recv.take() else {
            return;
        };
        let close_token = self.close_token.clone();
        let pending_bytes = self.pending_bytes.clone();
        let Some(mut writer) = self
            .network_writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        else {
            return;
        };
        let id = self.id;
        self.spawn_task(async move {
            loop {
                let recv_result = tokio::select! {
                    biased;
                    res = priority_packet_receiver.recv() => res,
                    res = packet_receiver.recv() => res,
                    () = close_token.cancelled() => {
                        priority_packet_receiver
                            .try_recv()
                            .ok()
                            .or_else(|| packet_receiver.try_recv().ok())
                    }
                };

                let Some(packet_data) = recv_result else {
                    break;
                };

                let mut packet_batch = Vec::with_capacity(MAX_BATCH_SIZE);
                packet_batch.push(packet_data);

                while packet_batch.len() < MAX_BATCH_SIZE {
                    match priority_packet_receiver.try_recv() {
                        Ok(packet_data) => {
                            packet_batch.push(packet_data);
                            continue;
                        }
                        Err(TryRecvError::Disconnected | TryRecvError::Empty) => {}
                    }

                    match packet_receiver.try_recv() {
                        Ok(packet_data) => packet_batch.push(packet_data),
                        Err(TryRecvError::Disconnected | TryRecvError::Empty) => break,
                    }
                }

                let mut packets_to_frame = VecDeque::from(packet_batch);
                let mut written_packets = Vec::with_capacity(packets_to_frame.len());
                let mut send_failed = false;

                while !packets_to_frame.is_empty() {
                    let frame_batch = take_frame_batch(&mut packets_to_frame);
                    let (returned_writer, returned_batch, frame, frame_err) =
                        match frame_batch_maybe_offload(writer, frame_batch).await {
                            Ok(result) => result,
                            Err(err) => {
                                if !close_token.is_cancelled() {
                                    warn!("客户端 {id} 的数据包组帧任务失败：{err}");
                                }
                                close_token.cancel();
                                return;
                            }
                        };
                    writer = returned_writer;

                    if let Some(err) = frame_err {
                        if !close_token.is_cancelled() {
                            warn!("客户端 {id} 的数据包组帧失败：{err}");
                        }
                        send_failed = true;
                        break;
                    }

                    if let Err(err) = writer.write_frame(&frame).await {
                        if !close_token.is_cancelled() {
                            warn!("向客户端 {id} 发送数据包批次失败：{err}");
                        }
                        send_failed = true;
                        break;
                    }

                    written_packets.extend(returned_batch);
                }

                if !send_failed && let Err(err) = writer.flush().await {
                    if !close_token.is_cancelled() {
                        warn!("为客户端 {id} 刷新数据包批次失败：{err}");
                    }
                    send_failed = true;
                }

                let flushed_bytes: usize = written_packets.iter().map(|p| p.data.len()).sum();
                decrement_pending_bytes(&pending_bytes, flushed_bytes);

                if send_failed {
                    // 现在我们需要关闭与客户端的连接，因为流处于未知状态。
                    close_token.cancel();
                    break;
                }

                for packet in written_packets {
                    if let Some(completion) = packet.completion {
                        let _ = completion.send(());
                    }
                }
            }
        });
    }

    /// 关闭与客户端的连接。
    ///
    /// 此函数使用原子标志将连接标记为已关闭。通常更推荐
    /// 如果想向客户端发送一条说明断开原因的特定消息，请使用 `kick` 函数。
    /// 不过，在发送消息并非关键或可能无法发送的场景（例如连接突然中断）中，请使用 `close`。
    ///
    /// # Notes
    ///
    /// 此函数不会尝试向客户端发送任何断开连接的数据包。
    pub fn close(&self) {
        self.close_token.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.close_token.is_cancelled()
    }

    #[expect(clippy::too_many_lines)]
    pub fn handle_play_packet(
        &self,
        player: &Arc<Player>,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<(), Box<dyn PapokinError>> {
        let version = self.version.load();

        let mut event = crate::plugin::server::packet::PacketReceivedEvent::new(
            player.clone(),
            packet.id,
            packet.payload.clone(),
        );
        server.plugin_manager.fire_blocking(server, &mut event);
        if event.cancelled {
            return Ok(());
        }

        let mut payload = &event.payload[..];
        match event.packet_id {
            id if id == SConfirmTeleport::to_id(version) => {
                self.handle_confirm_teleport(
                    player,
                    &SConfirmTeleport::read(&mut payload, &version)?,
                );
            }
            id if id == SChangeGameMode::to_id(version) => {
                self.handle_change_game_mode(
                    player,
                    &SChangeGameMode::read(&mut payload, &version)?,
                );
            }
            id if id == SChatAck::to_id(version) => {
                let packet = SChatAck::read(&mut payload, &version)?;
                self.handle_chat_ack(player, &packet);
            }
            id if id == SChatCommand::to_id(version) => {
                let packet = SChatCommand::read(&mut payload, &version)?;
                let cmd = packet.command.to_string();
                let client = player.client.clone();
                let player_c = player.clone();
                let server_c = server.clone();
                server.spawn_task(async move {
                    let packet = SChatCommand { command: &cmd };
                    client
                        .handle_chat_command(&player_c, &server_c, &packet)
                        .await;
                });
            }
            id if id == SChatCommandSigned::to_id(version) => {
                // 签名命令包同时携带 lastSeen 确认（原版客户端把确认捆绑在
                // 此包内），先应用到校验器再执行命令；解析失败的包不再宽容
                // 地按无签名命令执行，直接断开。
                let signed = SChatCommandSigned::read(&mut payload, &version)?;
                let cmd = signed.command.to_string();
                let ack = signed.acknowledged.to_vec();
                let count = signed.message_count;
                let checksum = signed.checksum;
                let client = player.client.clone();
                let player_c = player.clone();
                let server_c = server.clone();
                server.spawn_task(async move {
                    client.apply_signed_command_last_seen(
                        &server_c, &player_c, count, &ack, checksum,
                    );
                    let packet = SChatCommand { command: &cmd };
                    client
                        .handle_chat_command(&player_c, &server_c, &packet)
                        .await;
                });
            }
            id if id == SChatMessage::to_id(version) => {
                let packet = SChatMessage::read(&mut payload, &version)?;
                let msg = packet.message.to_string();
                let signature = packet.signature.map(<[u8]>::to_vec);
                let ack = packet.acknowledged.to_vec();
                let ts = packet.timestamp;
                let salt = packet.salt;
                let count = packet.message_count;
                let checksum = packet.checksum;
                let client = player.client.clone();
                let player_c = player.clone();
                let server_c = server.clone();
                server.spawn_task(async move {
                    let packet = SChatMessage {
                        message: &msg,
                        timestamp: ts,
                        salt,
                        signature: signature.as_deref(),
                        message_count: count,
                        acknowledged: &ack,
                        checksum,
                    };
                    client
                        .handle_chat_message(&server_c, &player_c, packet)
                        .await;
                });
            }
            id if id == SClientInformationPlay::to_id(version) => {
                let packet = SClientInformationPlay::read(&mut payload, &version)?;
                // 客户端选项变更通知（仅 play 阶段；
                // 配置阶段变体尚无玩家对象）。
                let mut options_event = crate::plugin::api::events::player::player_client_options_change::PlayerClientOptionsChangeEvent::new(
                    player.clone(),
                    packet.locale.to_string(),
                    i32::from(packet.view_distance),
                    match packet.chat_mode.0 {
                        0 => "enabled".to_string(),
                        1 => "commands_only".to_string(),
                        _ => "hidden".to_string(),
                    },
                    packet.chat_colors,
                    if packet.main_hand.0 == 0 {
                        "left".to_string()
                    } else {
                        "right".to_string()
                    },
                    u32::from(packet.skin_parts),
                );
                server
                    .plugin_manager
                    .fire_blocking(server, &mut options_event);
                self.handle_client_information(server, player, &packet);
            }
            id if id == SClientCommand::to_id(version) => {
                self.handle_client_status(player, &SClientCommand::read(&mut payload, &version)?);
            }
            id if id == SPlayerInput::to_id(version) => {
                self.handle_player_input(
                    player,
                    &SPlayerInput::read(&mut payload, &version)?,
                    server,
                );
            }
            id if id == SMoveVehicle::to_id(version) => {
                self.handle_move_vehicle(player, &SMoveVehicle::read(&mut payload, &version)?);
            }
            id if id == SPaddleBoat::to_id(version) => {
                self.handle_paddle_boat(player, &SPaddleBoat::read(&mut payload, &version)?);
            }
            id if id == SInteract::to_id(version) => {
                self.handle_interact(player, &SInteract::read(&mut payload, &version)?, server);
            }
            id if id == SBundleItemSelected::to_id(version) => {
                self.handle_bundle_item_selected(
                    player,
                    &SBundleItemSelected::read(&mut payload, &version)?,
                );
            }
            id if id == SAttack::to_id(version) => {
                self.handle_attack(player, &SAttack::read(&mut payload, &version)?, server);
            }
            id if id == STeleportToEntity::to_id(version) => {
                self.handle_teleport_to_entity(
                    player,
                    &STeleportToEntity::read(&mut payload, &version)?,
                    server,
                );
            }
            id if id == papokin_protocol::java::server::play::SKeepAlive::to_id(version) => {
                self.handle_keep_alive(
                    player,
                    &papokin_protocol::java::server::play::SKeepAlive::read(
                        &mut payload,
                        &version,
                    )?,
                );
            }
            id if id == SClientTickEnd::to_id(version) => {
                self.handle_client_tick_end(player);
                // 客户端刻结束通知（高频：依赖于
                // 调度器内部的零监听器提前返回）。
                let mut tick_event =
                    crate::plugin::api::events::player::client_tick_end::ClientTickEndEvent::new(
                        player.clone(),
                    );
                server.plugin_manager.fire_blocking(server, &mut tick_event);
            }
            id if id == STestInstanceBlockAction::to_id(version) => {
                self.handle_test_instance_block_action(
                    player,
                    &STestInstanceBlockAction::read(&mut payload, &version)?,
                );
            }
            id if id == SSetTestBlock::to_id(version) => {
                self.handle_set_test_block(player, &SSetTestBlock::read(&mut payload, &version)?);
            }
            id if id == SDebugSubscriptionRequest::to_id(version) => {
                self.handle_debug_subscription_request(
                    player,
                    &SDebugSubscriptionRequest::read(&mut payload, &version)?,
                );
            }
            id if id == SDebugSampleSubscription::to_id(version) => {
                self.handle_debug_sample_subscription(
                    player,
                    &SDebugSampleSubscription::read(&mut payload, &version)?,
                );
            }
            id if id == SPlayerPosition::to_id(version) => {
                self.handle_position(
                    player,
                    server,
                    &SPlayerPosition::read(&mut payload, &version)?,
                );
            }
            id if id == SPlayerPositionRotation::to_id(version) => {
                self.handle_position_rotation(
                    player,
                    server,
                    &SPlayerPositionRotation::read(&mut payload, &version)?,
                );
            }
            id if id == SPlayerRotation::to_id(version) => {
                self.handle_rotation(player, &SPlayerRotation::read(&mut payload, &version)?);
            }
            id if id == SSetPlayerGround::to_id(version) => {
                self.handle_player_ground(player, &SSetPlayerGround::read(&mut payload, &version)?);
            }
            id if id == SPickItemFromBlock::to_id(version) => {
                let packet = SPickItemFromBlock::read(&mut payload, &version)?;
                // 选取方块钩子：取消即否决此次选取。结果
                // 物品在此处计算（尽力而为）；guest 对它的修改
                // 的内容不会应用到选取逻辑上。
                let world = player.world();
                let block = world.get_block(&packet.pos);
                let result_item = papokin_data::item::Item::from_id(block.item_id)
                    .map(|item| papokin_data::item_stack::ItemStack::new(1, item));
                if let Some(result) = result_item {
                    let mut pick_event = crate::plugin::api::events::player::player_pick_block::PlayerPickBlockEvent::new(
                        player.clone(),
                        packet.pos,
                        result,
                    );
                    server.plugin_manager.fire_blocking(server, &mut pick_event);
                    if !pick_event.cancelled {
                        self.handle_pick_item_from_block(player, &packet);
                    }
                } else {
                    self.handle_pick_item_from_block(player, &packet);
                }
            }
            id if id
                == papokin_protocol::java::server::play::SPickItemFromEntity::to_id(version) =>
            {
                let packet = papokin_protocol::java::server::play::SPickItemFromEntity::read(
                    &mut payload,
                    &version,
                )?;
                // 选取实体钩子：取消即否决此次选取。结果是
                // 实体的刷怪蛋（若存在）；guest 侧的修改
                // 对它的修改不会应用到选取逻辑。
                let world = player.world();
                let result_item = world.get_entity_by_id(packet.id.0).and_then(|target| {
                    use papokin_data::entity::{entity_from_egg, spawn_egg_ids};
                    let target_type_id = target.get_entity().entity_type.id;
                    spawn_egg_ids().iter().find_map(|&egg_id| {
                        entity_from_egg(egg_id)
                            .filter(|et| et.id == target_type_id)
                            .and_then(|_| papokin_data::item::Item::from_id(egg_id))
                            .map(|item| papokin_data::item_stack::ItemStack::new(1, item))
                    })
                });
                if let Some(result) = result_item {
                    let mut pick_event = crate::plugin::api::events::player::player_pick_entity::PlayerPickEntityEvent::new(
                        player.clone(),
                        packet.id.0,
                        result,
                    );
                    server.plugin_manager.fire_blocking(server, &mut pick_event);
                    if !pick_event.cancelled {
                        self.handle_pick_item_from_entity(player, &packet);
                    }
                } else {
                    self.handle_pick_item_from_entity(player, &packet);
                }
            }
            id if id == SPlayerAbilities::to_id(version) => {
                self.handle_player_abilities(
                    player,
                    &SPlayerAbilities::read(&mut payload, &version)?,
                    server,
                );
            }
            id if id == SPlayerAction::to_id(version) => {
                self.handle_player_action(
                    player,
                    &SPlayerAction::read(&mut payload, &version)?,
                    server,
                );
            }
            id if id == SSetCommandBlock::to_id(version) => {
                self.handle_set_command_block(
                    player,
                    &SSetCommandBlock::read(&mut payload, &version)?,
                );
            }
            id if id == SSetJigsawBlock::to_id(version) => {
                self.handle_set_jigsaw_block(
                    player,
                    &SSetJigsawBlock::read(&mut payload, &version)?,
                );
            }
            id if id == SJigsawGenerate::to_id(version) => {
                self.handle_jigsaw_generate(
                    player,
                    &SJigsawGenerate::read(&mut payload, &version)?,
                );
            }
            id if id == SPlayerCommand::to_id(version) => {
                self.handle_player_command(
                    player,
                    &SPlayerCommand::read(&mut payload, &version)?,
                    server,
                );
            }
            id if id == SPlayerLoaded::to_id(version) => {
                Self::handle_player_loaded(player);
            }
            id if id == SPlayPingRequest::to_id(version) => {
                self.handle_play_ping_request(&SPlayPingRequest::read(&mut payload, &version)?);
            }
            id if id == SClickSlot::to_id(version) => {
                player.on_slot_click(SClickSlot::read(&mut payload, &version)?, server);
            }
            id if id == SContainerButtonClick::to_id(version) => {
                player.on_container_button_click(&SContainerButtonClick::read(
                    &mut payload,
                    &version,
                )?);
            }
            id if id == SSetHeldItem::to_id(version) => {
                self.handle_set_held_item(
                    server,
                    player,
                    &SSetHeldItem::read(&mut payload, &version)?,
                );
            }
            id if id == SSetCreativeSlot::to_id(version) => {
                self.handle_set_creative_slot(
                    player,
                    SSetCreativeSlot::read(&mut payload, &version)?,
                )?;
            }
            id if id == SSwingArm::to_id(version) => {
                self.handle_swing_arm(server, player, &SSwingArm::read(&mut payload, &version)?);
            }
            id if id == SUpdateSign::to_id(version) => {
                let packet = SUpdateSign::read(&mut payload, &version)?;
                // 未检查的符号变更钩子（原始行，校验之前）；
                // 取消会丢弃本次更新。已通过校验的 SignChangeEvent
                // handle_sign_update 内部的处理仍独立运行。
                let mut sign_event = crate::plugin::api::events::player::unchecked_sign_change::UncheckedSignChangeEvent::new(
                    player.clone(),
                    packet.location,
                    vec![
                        packet.line_1.to_string(),
                        packet.line_2.to_string(),
                        packet.line_3.to_string(),
                        packet.line_4.to_string(),
                    ],
                );
                server.plugin_manager.fire_blocking(server, &mut sign_event);
                if !sign_event.cancelled {
                    self.handle_sign_update(player, &packet);
                }
            }
            id if id == SEditBook::to_id(version) => {
                self.handle_edit_book(player, &SEditBook::read(&mut payload, &version)?);
            }
            id if id == SUseItemOn::to_id(version) => {
                self.handle_use_item_on(
                    player,
                    &SUseItemOn::read(&mut payload, &version)?,
                    server,
                )?;
            }
            id if id == SUseItem::to_id(version) => {
                self.handle_use_item(player, &SUseItem::read(&mut payload, &version)?, server);
            }
            id if id == SCommandSuggestion::to_id(version) => {
                self.handle_command_suggestion(
                    player,
                    &SCommandSuggestion::read(&mut payload, &version)?,
                    server,
                );
            }
            id if id == SPCookieResponse::to_id(version) => {
                self.handle_cookie_response(&SPCookieResponse::read(&mut payload, &version)?);
            }
            id if id == SCloseContainer::to_id(version) => {
                let _ = SCloseContainer::read(&mut payload, &version)?;
                self.handle_close_container(player);
            }
            id if id == SChunkBatch::to_id(version) => {
                self.handle_chunk_batch(player, &SChunkBatch::read(&mut payload, &version)?);
            }
            id if id == SPlayerSession::to_id(version) => {
                let session = SPlayerSession::read(&mut payload, &version)?;
                let client = player.client.clone();
                let player_c = player.clone();
                let server_c = server.clone();
                server.spawn_task(async move {
                    client
                        .handle_chat_session_update(&player_c, &server_c, session)
                        .await;
                });
            }
            id if id == SCustomPayload::to_id(version) => {
                let payload = SCustomPayload::read(&mut payload, &version)?;
                let channel_str = payload.channel.to_string();
                let mut event = PlayerCustomPayloadEvent::new(
                    player.clone(),
                    channel_str.clone(),
                    Bytes::copy_from_slice(payload.data),
                );
                server.plugin_manager.fire_blocking(server, &mut event);

                // 分发到注册了此消息通道的插件。
                if !server
                    .plugin_manager
                    .get_channel_handlers(&channel_str)
                    .is_empty()
                {
                    let channel = channel_str.clone();
                    let player_uuid = player.gameprofile.id;
                    let data = event.data.to_vec();
                    let plugin_manager = server.plugin_manager.clone();
                    server.spawn_task(async move {
                        plugin_manager
                            .dispatch_plugin_message(&channel, player_uuid, data)
                            .await;
                    });
                }

                if channel_str == "minecraft:register" {
                    if let Ok(channels_data) = std::str::from_utf8(payload.data) {
                        for ch in channels_data.split('\0') {
                            if !ch.is_empty() {
                                let mut reg_event = crate::plugin::api::events::player::player_register_channel::PlayerRegisterChannelEvent::new(
                                    player.clone(),
                                    ch.to_string(),
                                );
                                server.plugin_manager.fire_blocking(server, &mut reg_event);
                                let mut ch_event = crate::plugin::api::events::player::player_channel::PlayerChannelEvent {
                                    player: player.clone(),
                                    channel: ch.to_string(),
                                    cancelled: false,
                                };
                                server.plugin_manager.fire_blocking(server, &mut ch_event);
                            }
                        }
                    }
                } else if channel_str == "minecraft:unregister"
                    && let Ok(channels_data) = std::str::from_utf8(payload.data)
                {
                    for ch in channels_data.split('\0') {
                        if !ch.is_empty() {
                            let mut unreg_event = crate::plugin::api::events::player::player_unregister_channel::PlayerUnregisterChannelEvent::new(
                                player.clone(),
                                ch.to_string(),
                            );
                            server
                                .plugin_manager
                                .fire_blocking(server, &mut unreg_event);
                        }
                    }
                }
            }
            id if id == SRecipeBookChangeSettings::to_id(version) => {
                self.handle_recipe_book_change_settings(
                    server,
                    player,
                    &SRecipeBookChangeSettings::read(&mut payload, &version)?,
                );
            }
            id if id == SRecipeBookSeenRecipe::to_id(version) => {
                self.handle_recipe_book_seen_recipe(
                    server,
                    player,
                    &SRecipeBookSeenRecipe::read(&mut payload, &version)?,
                );
            }
            id if id == SRenameItem::to_id(version) => {
                player.on_rename_item(&SRenameItem::read(&mut payload, &version)?);
            }
            id if id == SPlaceRecipe::to_id(version) => {
                let packet = SPlaceRecipe::read(&mut payload, &version)?;
                self.handle_place_recipe(server, player, &packet);
            }
            id if id
                == papokin_protocol::java::server::play::SCustomClickAction::to_id(version) =>
            {
                let packet = papokin_protocol::java::server::play::SCustomClickAction::read(
                    &mut payload,
                    &version,
                )?;
                let mut event = crate::plugin::api::events::dialog::dialog_click_action::DialogClickActionEvent::new(
                    player.clone(),
                    packet.action_id.to_string(),
                    packet.payload.map(Bytes::copy_from_slice),
                );
                server.plugin_manager.fire_blocking(server, &mut event);
            }
            id if id == SSelectTrade::to_id(version) => {
                self.handle_select_trade(player, &SSelectTrade::read(&mut payload, &version)?);
            }
            id if id == SSeenAdvancement::to_id(version) => {
                self.handle_seen_advancement(
                    player,
                    &SSeenAdvancement::read(&mut payload, &version)?,
                );
            }
            id if id == SPlayResourcePack::to_id(version) => {
                self.handle_play_resource_pack_response(
                    server,
                    player,
                    &SPlayResourcePack::read(&mut payload, &version)?,
                );
            }
            id if id == SPlayPong::to_id(version) => {
                self.handle_play_pong(player, &SPlayPong::read(&mut payload, &version)?);
            }
            id if id == SLockDifficulty::to_id(version) => {
                self.handle_lock_difficulty(
                    server,
                    player,
                    &SLockDifficulty::read(&mut payload, &version)?,
                );
            }
            id if id == SChangeDifficulty::to_id(version) => {
                self.handle_change_difficulty(
                    server,
                    player,
                    &SChangeDifficulty::read(&mut payload, &version)?,
                );
            }
            id if id == SSetBeacon::to_id(version) => {
                self.handle_set_beacon(player, &SSetBeacon::read(&mut payload, &version)?);
            }
            id if id == SContainerSlotStateChanged::to_id(version) => {
                self.handle_container_slot_state_changed(
                    player,
                    &SContainerSlotStateChanged::read(&mut payload, &version)?,
                );
            }
            id if id == SSpectateEntity::to_id(version) => {
                self.handle_spectate_entity(
                    player,
                    server,
                    &SSpectateEntity::read(&mut payload, &version)?,
                );
            }
            id if id == SSetCommandMinecart::to_id(version) => {
                self.handle_set_command_minecart(
                    player,
                    &SSetCommandMinecart::read(&mut payload, &version)?,
                );
            }
            id if id == SSetStructureBlock::to_id(version) => {
                self.handle_set_structure_block(
                    player,
                    &SSetStructureBlock::read(&mut payload, &version)?,
                );
            }
            id if id == SSetGameRule::to_id(version) => {
                self.handle_set_game_rule(player, &SSetGameRule::read(&mut payload, &version)?);
            }
            id if id == SBlockEntityTagQuery::to_id(version) => {
                self.handle_block_entity_tag_query(
                    player,
                    &SBlockEntityTagQuery::read(&mut payload, &version)?,
                );
            }
            id if id == SEntityTagQuery::to_id(version) => {
                self.handle_entity_tag_query(
                    player,
                    &SEntityTagQuery::read(&mut payload, &version)?,
                );
            }
            id if id == SConfigurationAcknowledged::to_id(version) => {
                self.handle_configuration_acknowledged(player);
            }
            _ => {
                warn!("无法处理 id 为 {} 的玩家数据包", event.packet_id);
            }
        }
        Ok(())
    }
}
