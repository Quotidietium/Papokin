use std::{
    net::{IpAddr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use dashmap::DashMap;

use packet::{ClientboundPacket, Packet, PacketError, ServerboundPacket};
use papokin_config::RCONConfig;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    select,
};
use tracing::{debug, error, info, warn};

use crate::command::CommandSender;
use crate::{SHOULD_STOP, STOP_INTERRUPT, server::Server};

pub use papokin_protocol::rcon as packet;

/// 恒时字节序列比较：无论在哪一位出现差异，耗时一致，
/// 避免认证比较的时序侧信道（长度先行为门是标准折中，
/// 长度本身不承载秘密）。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 连续认证失败达到该次数后，来源 IP 进入冷却封锁。
const AUTH_FAILURE_LIMIT: u32 = 5;
/// 冷却封锁时长：期间新连接直接断开，不给爆破者任何响应反馈。
const AUTH_BLOCK_DURATION: Duration = Duration::from_secs(300);

#[derive(Clone, Copy)]
struct AuthFailState {
    failures: u32,
    blocked_until: Option<Instant>,
}

/// RCON 认证节流器：按 IP 统计认证失败，抑制在线暴力破解。
/// 认证失败即断连但不限制重连的原版行为允许无限速试密码。
struct AuthThrottle {
    states: DashMap<IpAddr, AuthFailState>,
}

impl AuthThrottle {
    fn new() -> Self {
        Self {
            states: DashMap::new(),
        }
    }

    /// 该 IP 是否处于冷却封锁期。
    fn is_blocked(&self, ip: &IpAddr) -> bool {
        self.states.get(ip).is_some_and(|state| {
            state
                .blocked_until
                .is_some_and(|until| Instant::now() < until)
        })
    }

    /// 认证成功：清零该 IP 的失败计数。
    fn record_success(&self, ip: &IpAddr) {
        self.states.remove(ip);
    }

    /// 记录一次认证失败；返回该 IP 是否因此进入封锁。
    fn record_failure(&self, ip: &IpAddr) -> bool {
        let mut state = self.states.entry(*ip).or_insert(AuthFailState {
            failures: 0,
            blocked_until: None,
        });
        state.failures += 1;
        if state.failures >= AUTH_FAILURE_LIMIT {
            state.blocked_until = Some(Instant::now() + AUTH_BLOCK_DURATION);
            state.failures = 0;
            true
        } else {
            false
        }
    }

    /// 清理已过期的封锁条目，防止状态表随来源 IP 无界增长。
    fn retain_active(&self) {
        self.states
            .retain(|_, state| state.blocked_until.is_some_and(|t| Instant::now() < t));
    }
}

pub struct RCONServer;

impl RCONServer {
    pub async fn run(config: &RCONConfig, server: Arc<Server>) {
        if config.password.trim().is_empty() {
            error!("RCON 已启用但密码为空！出于安全考虑拒绝启动 RCON 服务器。");
            return;
        }

        let listener = match tokio::net::TcpListener::bind(config.address).await {
            Ok(l) => l,
            Err(e) => {
                error!("RCON 服务器绑定 {} 失败：{e}", config.address);
                return;
            }
        };

        info!("RCON 服务器正在监听 {}", config.address);

        let password = Arc::new(config.password.clone());
        let connections = Arc::new(AtomicU32::new(0));
        let throttle = Arc::new(AuthThrottle::new());
        let mut last_gc = Instant::now();

        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let await_new_client = || async {
                let t1 = listener.accept();
                let t2 = STOP_INTERRUPT.cancelled();

                select! {
                    client = t1 => Some(client),
                    () = t2 => None,
                }
            };
            // 异步等待入站套接字。

            let Some(Ok((connection, address))) = await_new_client().await else {
                break;
            };

            if last_gc.elapsed() >= Duration::from_secs(60) {
                throttle.retain_active();
                last_gc = Instant::now();
            }

            let current_conns = connections.load(Ordering::Relaxed);
            if config.max_connections != 0 && current_conns >= config.max_connections {
                warn!(
                    "RCON ({}): 连接被拒绝，已达最大连接数 ({})",
                    address, config.max_connections
                );
                continue;
            }

            connections.fetch_add(1, Ordering::Relaxed);
            let mut client = RCONClient::new(connection, address);

            let password = password.clone();
            let server = server.clone();
            let connections = connections.clone();
            let throttle = throttle.clone();
            tokio::spawn(async move {
                while !client.handle(&server, &password, &throttle).await {}
                connections.fetch_sub(1, Ordering::Relaxed);
                if server.advanced_config.networking.rcon.logging.quit {
                    info!("RCON ({}): 客户端已断开连接", address);
                }
                debug!("已关闭与 {} 的 RCON 连接", address);
            });
        }
    }
}

pub struct RCONClient {
    connection: tokio::net::TcpStream,
    address: SocketAddr,
    logged_in: bool,
    incoming: Vec<u8>,
    closed: bool,
}

impl RCONClient {
    #[must_use]
    pub const fn new(connection: tokio::net::TcpStream, address: SocketAddr) -> Self {
        Self {
            connection,
            address,
            logged_in: false,
            incoming: Vec::new(),
            closed: false,
        }
    }

    /// 返回客户端是否已关闭。仅由本模块的 accept 循环调用，
    /// 不对外暴露（签名含模块私有的 `AuthThrottle`）。
    async fn handle(
        &mut self,
        server: &Arc<Server>,
        password: &str,
        throttle: &AuthThrottle,
    ) -> bool {
        if !self.closed {
            match self.read_bytes().await {
                // 流已经关闭，我们无法回复，所以干脆把一切都关闭。
                Ok(true) => return true,
                Ok(false) => {}
                Err(e) => {
                    error!("无法读取数据包：{e}");
                    return true;
                }
            }
            while !self.closed {
                match self.receive_packet() {
                    Ok(Some(packet)) => {
                        if let Err(e) = self
                            .process_packet(server, password, throttle, packet)
                            .await
                        {
                            error!("RCON 错误：{e}");
                            self.closed = true;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        error!("RCON 数据包错误：{e}");
                        self.closed = true;
                        break;
                    }
                }
            }
        }
        self.closed
    }

    async fn process_packet(
        &mut self,
        server: &Arc<Server>,
        password: &str,
        throttle: &AuthThrottle,
        packet: Packet,
    ) -> Result<(), PacketError> {
        let config = &server.advanced_config.networking.rcon;
        match packet.get_type() {
            ServerboundPacket::Auth => {
                // 冷却封锁期内直接断开：不比较、不响应，不给爆破者反馈
                let ip = self.address.ip();
                if throttle.is_blocked(&ip) {
                    self.closed = true;
                    return Ok(());
                }
                if constant_time_eq(packet.get_body().as_bytes(), password.as_bytes()) {
                    throttle.record_success(&ip);
                    self.send(ClientboundPacket::AuthResponse, packet.get_id(), "")
                        .await?;
                    if config.logging.logged_successfully {
                        info!("RCON ({}): 客户端登录成功", self.address);
                    }
                    self.logged_in = true;
                } else {
                    let blocked = throttle.record_failure(&ip);
                    if config.logging.wrong_password {
                        info!("RCON ({}): 客户端尝试了错误的密码", self.address);
                    }
                    if blocked {
                        warn!(
                            "RCON ({}): 连续认证失败达 {AUTH_FAILURE_LIMIT} 次，封禁 {AUTH_BLOCK_DURATION:?}",
                            self.address
                        );
                        self.closed = true;
                        return Ok(());
                    }
                    self.send(ClientboundPacket::AuthResponse, -1, "").await?;
                    self.closed = true;
                }
            }
            ServerboundPacket::ExecCommand => {
                if self.logged_in {
                    let output = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
                    let packet_body = packet.get_body().to_owned();

                    let command_source = CommandSender::Rcon(output.clone()).into_source(server);

                    server
                        .command_dispatcher
                        .load()
                        .handle_command(&command_source, &packet_body);

                    let output_lines: Vec<String> = output
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    if output_lines.is_empty() {
                        if config.logging.commands {
                            info!("RCON ({}): 已执行命令：{}", self.address, packet.get_body());
                        }
                        self.send(ClientboundPacket::Output, packet.get_id(), "")
                            .await?;
                    } else {
                        for line in &output_lines {
                            if config.logging.commands {
                                info!("RCON ({}): {}", self.address, line);
                            }
                            self.send(ClientboundPacket::Output, packet.get_id(), line)
                                .await?;
                        }
                    }
                } else {
                    if config.logging.wrong_password {
                        info!("RCON ({}): 未认证的客户端试图执行命令", self.address);
                    }
                    self.send(ClientboundPacket::AuthResponse, -1, "").await?;
                    self.closed = true;
                }
            }
        }
        Ok(())
    }

    async fn read_bytes(&mut self) -> std::io::Result<bool> {
        let mut buf = [0; 1460];
        let n = self.connection.read(&mut buf).await?;
        if n == 0 {
            return Ok(true);
        }
        self.incoming.extend_from_slice(&buf[..n]);
        Ok(false)
    }

    async fn send(
        &mut self,
        packet: ClientboundPacket,
        id: i32,
        body: &str,
    ) -> Result<(), PacketError> {
        let buf = packet.write_buf(id, body);
        self.connection
            .write_all(&buf)
            .await
            .map_err(PacketError::FailedSend)?;
        Ok(())
    }

    fn receive_packet(&mut self) -> Result<Option<Packet>, PacketError> {
        Packet::deserialize(&mut self.incoming)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_compares_correctly() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secre"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn throttle_blocks_after_limit_and_clears_on_success() {
        let throttle = AuthThrottle::new();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();

        for i in 1..AUTH_FAILURE_LIMIT {
            assert!(!throttle.record_failure(&ip), "第 {i} 次失败不应封锁");
        }
        assert!(throttle.record_failure(&ip), "达到上限必须封锁");
        assert!(throttle.is_blocked(&ip));

        // 成功登录清零计数，立即解封
        throttle.record_success(&ip);
        assert!(!throttle.is_blocked(&ip));
        assert!(!throttle.record_failure(&ip));
    }

    #[test]
    fn throttle_expiry_releases_block() {
        let throttle = AuthThrottle::new();
        let ip: IpAddr = "203.0.113.8".parse().unwrap();
        for _ in 0..AUTH_FAILURE_LIMIT {
            throttle.record_failure(&ip);
        }
        assert!(throttle.is_blocked(&ip));

        // 手动把封锁截止时间拨到过去（同模块测试可访问私有字段）
        *throttle.states.get_mut(&ip).unwrap() = AuthFailState {
            failures: 0,
            blocked_until: Instant::now().checked_sub(Duration::from_secs(1)),
        };
        assert!(!throttle.is_blocked(&ip));
    }

    #[test]
    fn throttle_states_are_isolated_per_ip() {
        let throttle = AuthThrottle::new();
        let a: IpAddr = "203.0.113.9".parse().unwrap();
        let b: IpAddr = "203.0.113.10".parse().unwrap();
        for _ in 0..AUTH_FAILURE_LIMIT {
            throttle.record_failure(&a);
        }
        assert!(throttle.is_blocked(&a));
        assert!(!throttle.is_blocked(&b));
    }
}
