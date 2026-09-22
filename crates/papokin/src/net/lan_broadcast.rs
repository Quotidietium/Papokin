use papokin_config::LANBroadcastConfig;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::{select, time};
use tracing::{error, info, warn};

use crate::{SHOULD_STOP, STOP_INTERRUPT};

/// Minecraft 用于局域网发现的标准多播地址
///
/// Java 版使用此特定的多播组来“广播”服务器
/// 向同一局域网内的客户端广播自身存在
const BROADCAST_ADDRESS: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 2, 60)), 4445);

pub struct LANBroadcast {
    port: u16,
    motd: String,
}

impl LANBroadcast {
    /// 根据提供的配置创建新的局域网广播实例
    #[must_use]
    pub fn new(config: &LANBroadcastConfig, server_motd: &str) -> Self {
        let port = config.port.unwrap_or(0);

        let advanced_motd = config.motd.clone().unwrap_or_default();

        let motd = if advanced_motd.is_empty() {
            warn!(
                "正在将服务器 MOTD 用作 LAN 广播 MOTD。注意：LAN 广播 MOTD 不支持多行、RGB 颜色或渐变，建议据此单独配置。"
            );
            server_motd.replace('\n', " ")
        } else {
            advanced_motd
        };

        Self { port, motd }
    }

    /// 启动 UDP 广播循环。应在独立任务中运行
    ///
    /// 此循环每 1.5 秒发送一个包含 MOTD 和
    /// 实际游戏服务器监听的端口。
    ///
    /// # Arguments
    /// * `bound_addr` - 实际 Minecraft 服务器运行所在的地址
    ///   客户端将使用此地址的端口来进行连接
    ///
    /// # Panics
    /// 如果 UDP 套接字无法绑定或广播权限被拒绝则 panic
    pub async fn start(self, bound_addr: SocketAddr) {
        let Ok(socket) = UdpSocket::bind(format!("0.0.0.0:{}", self.port)).await else {
            error!("无法绑定 LAN 广播 UDP 套接字");
            return;
        };

        if let Err(err) = socket.set_broadcast(true) {
            error!("无法设置 LAN 广播：{err}");
            return;
        }

        let mut interval = time::interval(Duration::from_millis(1500));

        let advertisement = format!("[MOTD]{}[/MOTD][AD]{}[/AD]", self.motd, bound_addr.port());

        if let Ok(local_addr) = socket.local_addr() {
            info!("LAN 广播正在 {local_addr} 上运行");
        }

        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let t1 = interval.tick();
            let t2 = STOP_INTERRUPT.cancelled();

            let should_continue = select! {
                _ = t1 => true,
                () = t2 => false,
            };

            if !should_continue {
                break;
            }

            let _ = socket
                .send_to(advertisement.as_bytes(), BROADCAST_ADDRESS)
                .await;
        }
    }
}
