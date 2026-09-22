use std::{
    collections::HashMap,
    ffi::{CString, NulError},
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use papokin_protocol::query::{
    CBasicStatus, CFullStatus, CHandshake, PacketType, RawQueryPacket, SHandshake, SStatusRequest,
};
use papokin_util::text::{TextComponent, color::NamedColor};
use papokin_world::CURRENT_MC_VERSION;
use rand::RngExt;
use tokio::{net::UdpSocket, sync::RwLock, time};
use tracing::{error, info};

use crate::{SHOULD_STOP, STOP_INTERRUPT, server::Server};

pub async fn start_query_handler(server: Arc<Server>, query_addr: SocketAddr) {
    let Ok(socket) = UdpSocket::bind(query_addr).await else {
        error!("无法绑定 Query UDP 套接字");
        return;
    };
    let socket = Arc::new(socket);

    // 挑战令牌绑定到 IP 地址和端口
    let valid_challenge_tokens = Arc::new(RwLock::new(HashMap::new()));
    let valid_challenge_tokens_clone = valid_challenge_tokens.clone();
    // 所有已创建的挑战令牌每 30 秒全部过期
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;
            valid_challenge_tokens_clone.write().await.clear();
        }
    });

    if let Ok(local_addr) = socket.local_addr() {
        info!(
            "服务器 Query 正在端口 {} 上运行",
            TextComponent::text(format!("{}", local_addr.port()))
                .color_named(NamedColor::DarkBlue)
                .to_pretty_console()
        );
    }

    while !SHOULD_STOP.load(Ordering::Relaxed) {
        let socket = socket.clone();
        let valid_challenge_tokens = valid_challenge_tokens.clone();
        let server = server.clone();
        let mut buf = vec![0; 1024];

        let recv_result = tokio::select! {
            result = socket.recv_from(&mut buf) => Some(result),
            () = STOP_INTERRUPT.cancelled() => None,
        };

        let Some(Ok((length, addr))) = recv_result else {
            break;
        };

        buf.truncate(length);

        tokio::spawn(async move {
            if let Err(err) = handle_packet(
                buf,
                valid_challenge_tokens,
                server,
                socket,
                addr,
                query_addr,
            )
            .await
            {
                error!("发现内嵌 0 字节！无法编码 Query 响应！{err}");
            }
        });
    }
}

// 不符合格式的数据包的错误不会被返回，因为我们反正也不会处理它们
// 唯一会抛出的错误源于 CString 中的空终止符
// 因为这些错误需要由服务器所有者修正
#[expect(clippy::too_many_lines)]
#[inline]
async fn handle_packet(
    buf: Vec<u8>,
    clients: Arc<RwLock<HashMap<i32, SocketAddr>>>,
    server: Arc<Server>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    bound_addr: SocketAddr,
) -> Result<(), NulError> {
    if let Ok(mut raw_packet) = RawQueryPacket::decode(buf).await {
        match raw_packet.packet_type {
            PacketType::Handshake => {
                if let Ok(packet) = SHandshake::decode(&mut raw_packet).await {
                    let challenge_token = rand::rng().random_range(1..=i32::MAX);
                    let response = CHandshake {
                        session_id: packet.session_id,
                        challenge_token,
                    };

                    // 忽略所有错误，因为我们不想让查询处理器崩溃
                    // 协议同样忽略所有错误并且不响应
                    if let Some(encoded) = response.encode() {
                        let _ = socket.send_to(encoded.as_slice(), addr).await;
                    }

                    clients.write().await.insert(challenge_token, addr);
                }
            }
            PacketType::Status => {
                if let Ok(packet) = SStatusRequest::decode(&mut raw_packet).await
                    && clients
                        .read()
                        .await
                        .get(&packet.challenge_token)
                        .is_some_and(|token_bound_ip: &SocketAddr| token_bound_ip == &addr)
                {
                    if packet.is_full_request {
                        // 获取 4 名玩家
                        let mut players: Vec<CString> = Vec::new();
                        for world in server.worlds.load().iter() {
                            let mut world_players = world
                                .players
                                .load()
                                // 虽然没有文档记载的限制，但我们将限制为 4 名玩家
                                .iter()
                                .take(4 - players.len())
                                .filter_map(|player| {
                                    CString::new(player.gameprofile.name.as_str()).ok()
                                })
                                .collect::<Vec<_>>();

                            players.append(&mut world_players); // 追加来自此世界的玩家

                            if players.len() >= 4 {
                                break; // 如果已收集到 4 名玩家则停止
                            }
                        }

                        let plugins = server
                            .plugin_manager
                            .active_plugins()
                            .into_iter()
                            .map(|meta| meta.name)
                            .reduce(|acc, name| format!("{acc}, {name}"))
                            .unwrap_or_default();

                        // GS4 Query 钩子（完整状态）：携带通知
                        // 收集到的响应数据。
                        let mut query_event =
                            crate::plugin::api::events::player::gs4_query::Gs4QueryEvent::new(
                                "full",
                                addr.to_string(),
                                vec![
                                    (
                                        "hostname".to_string(),
                                        server.advanced_config.networking.java.motd.clone(),
                                    ),
                                    ("version".to_string(), CURRENT_MC_VERSION.to_string()),
                                    ("plugins".to_string(), plugins.clone()),
                                    (
                                        "num_players".to_string(),
                                        server.get_player_count().to_string(),
                                    ),
                                    (
                                        "max_players".to_string(),
                                        server
                                            .advanced_config
                                            .networking
                                            .java
                                            .max_players
                                            .to_string(),
                                    ),
                                ],
                            );
                        server.plugin_manager.fire(&server, &mut query_event).await;

                        let response = CFullStatus {
                            session_id: packet.session_id,
                            hostname: CString::new(
                                server.advanced_config.networking.java.motd.as_str(),
                            )?,
                            version: CString::new(CURRENT_MC_VERSION)?,
                            plugins: CString::new(plugins)?,
                            map: CString::new(
                                server
                                    .worlds
                                    .load()
                                    .first()
                                    .map_or("world", |w| w.get_world_name()),
                            )?,
                            num_players: server.get_player_count(),
                            max_players: server.advanced_config.networking.java.max_players
                                as usize,
                            host_port: bound_addr.port(),
                            host_ip: CString::new(bound_addr.ip().to_string())?,
                            players,
                        };

                        if let Some(encoded) = response.encode() {
                            let _ = socket.send_to(encoded.as_slice(), addr).await;
                        }
                    } else {
                        // GS4 Query 钩子（基本状态）：携带通知
                        // 收集到的响应数据。
                        let mut query_event =
                            crate::plugin::api::events::player::gs4_query::Gs4QueryEvent::new(
                                "basic",
                                addr.to_string(),
                                vec![
                                    (
                                        "motd".to_string(),
                                        server.advanced_config.networking.java.motd.clone(),
                                    ),
                                    (
                                        "num_players".to_string(),
                                        server.get_player_count().to_string(),
                                    ),
                                    (
                                        "max_players".to_string(),
                                        server
                                            .advanced_config
                                            .networking
                                            .java
                                            .max_players
                                            .to_string(),
                                    ),
                                ],
                            );
                        server.plugin_manager.fire(&server, &mut query_event).await;

                        let response = CBasicStatus {
                            session_id: packet.session_id,
                            motd: CString::new(
                                server.advanced_config.networking.java.motd.as_str(),
                            )?,
                            map: CString::new(
                                server
                                    .worlds
                                    .load()
                                    .first()
                                    .map_or("world", |w| w.get_world_name()),
                            )?,
                            num_players: server.get_player_count(),
                            max_players: server.advanced_config.networking.java.max_players
                                as usize,
                            host_port: bound_addr.port(),
                            host_ip: CString::new(bound_addr.ip().to_string())?,
                        };

                        if let Some(encoded) = response.encode() {
                            let _ = socket.send_to(encoded.as_slice(), addr).await;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
