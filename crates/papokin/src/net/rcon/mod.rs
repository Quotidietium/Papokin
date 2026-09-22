use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

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
            tokio::spawn(async move {
                while !client.handle(&server, &password).await {}
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

    /// 返回客户端是否已关闭。
    pub async fn handle(&mut self, server: &Arc<Server>, password: &str) -> bool {
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
                        if let Err(e) = self.process_packet(server, password, packet).await {
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
        packet: Packet,
    ) -> Result<(), PacketError> {
        let config = &server.advanced_config.networking.rcon;
        match packet.get_type() {
            ServerboundPacket::Auth => {
                if !password.is_empty() && packet.get_body() == password {
                    self.send(ClientboundPacket::AuthResponse, packet.get_id(), "")
                        .await?;
                    if config.logging.logged_successfully {
                        info!("RCON ({}): 客户端登录成功", self.address);
                    }
                    self.logged_in = true;
                } else {
                    if config.logging.wrong_password {
                        info!("RCON ({}): 客户端尝试了错误的密码", self.address);
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
