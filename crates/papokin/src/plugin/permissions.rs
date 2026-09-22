/// 允许插件执行 DNS 解析（将主机名解析为 IP 地址）。
///
/// 对应 `wasi:sockets/ip-name-lookup` 接口。
pub const NETWORK_DNS: &str = "network.dns";

/// 允许插件使用 TCP 套接字。
pub const NETWORK_TCP: &str = "network.tcp";

/// 允许插件使用 UDP 套接字。
pub const NETWORK_UDP: &str = "network.udp";

/// 允许插件发起 TCP 连接。
pub const NETWORK_TCP_CONNECT: &str = "network.tcp.connect";

/// 允许插件绑定 TCP 监听器（接受入站连接）。
pub const NETWORK_TCP_BIND: &str = "network.tcp.bind";

/// 允许插件向指定目标收发 UDP 数据包。
pub const NETWORK_UDP_CONNECT: &str = "network.udp.connect";

/// 允许插件将 UDP 套接字绑定到本地端口。
pub const NETWORK_UDP_BIND: &str = "network.udp.bind";

/// 允许插件在未连接的 UDP 套接字上发送数据报。
pub const NETWORK_UDP_OUTGOING_DATAGRAM: &str = "network.udp.outgoingdatagram";

/// 将所有网络权限限制为仅限环回地址（localhost）。
pub const NETWORK_LOOPBACK: &str = "network.loopback";

/// 允许插件发起出站 TCP/UDP 连接。
///
/// 这让插件能够完全访问宿主的网络接口。
/// **警告：** 这是一项强大的权限，只应授予受信任的插件。
pub const NETWORK_OUTBOUND: &str = "network.outbound";

/// 允许插件发起出站 HTTP 连接。
///
/// 这与 `network.outbound` 是分开的。此项允许使用 `wasi:http`；另一项则允许使用更强大的 `wasi:sockets`。
pub const HTTP_OUTBOUND: &str = "http.outbound";

/// 允许插件读取自己数据文件夹（`plugins/data/<name>`）内的文件。
pub const FS_READ_DATA: &str = "fs.read.data";

/// 允许插件在自己的数据文件夹（`plugins/data/<name>`）内写（和读）文件。
///
/// 注意：此权限同时隐含 `FS_READ_DATA`
pub const FS_WRITE_DATA: &str = "fs.write.data";

/// 允许插件读取所有环境变量。
pub const SYS_ENV: &str = "sys.env";

/// 允许插件读取指定的环境变量。
/// 与 "sys.env.PATH" 这样的前缀一起使用。
pub const SYS_ENV_PREFIX: &str = "sys.env.";

/// 允许插件读取系统信息（CPU、内存、操作系统）。
pub const SYS_INFO: &str = "sys.info";

/// 允许插件读取 CPU 信息。
pub const SYS_INFO_CPU: &str = "sys.info.cpu";

/// 允许插件读取内存信息。
pub const SYS_INFO_RAM: &str = "sys.info.ram";

/// 允许插件读取操作系统信息。
pub const SYS_INFO_OS: &str = "sys.info.os";

#[must_use]
pub fn get_permission_description(permission: &str) -> Option<&'static str> {
    match permission {
        NETWORK_DNS => Some("允许插件进行 DNS 解析（将主机名解析为 IP 地址）。"),
        NETWORK_TCP => Some("允许插件使用 TCP 套接字。"),
        NETWORK_UDP => Some("允许插件使用 UDP 套接字。"),
        NETWORK_TCP_CONNECT => Some("允许插件发起 TCP 连接。"),
        NETWORK_TCP_BIND => Some("允许插件绑定 TCP 监听器（接受入站连接）。"),
        NETWORK_UDP_CONNECT => Some("允许插件向指定目标发送和接收 UDP 数据包。"),
        NETWORK_UDP_BIND => Some("允许插件将 UDP 套接字绑定到本地端口。"),
        NETWORK_UDP_OUTGOING_DATAGRAM => Some("允许插件在未连接的 UDP 套接字上发送数据报。"),
        NETWORK_LOOPBACK => Some("将所有网络权限限制为仅回环地址（localhost）。"),
        NETWORK_OUTBOUND => Some("允许插件发起出站 TCP/UDP 连接。（高权限）"),
        HTTP_OUTBOUND => Some("允许插件发起出站 HTTP 请求（通过 `wasi:http`）。"),
        FS_READ_DATA => Some("允许插件读取其自身数据文件夹内的文件。"),
        FS_WRITE_DATA => {
            Some("允许插件写入（和读取）其自身数据文件夹内的文件。隐含 `fs.read.data`。")
        }
        SYS_ENV => Some("允许插件读取所有环境变量。"),
        SYS_INFO => Some("允许插件读取系统信息（CPU、内存、操作系统）。"),
        SYS_INFO_CPU => Some("允许插件读取 CPU 信息。"),
        SYS_INFO_RAM => Some("允许插件读取内存信息。"),
        SYS_INFO_OS => Some("允许插件读取操作系统信息。"),
        p if p.starts_with(SYS_ENV_PREFIX) => Some("允许插件读取指定的环境变量。"),
        _ => None,
    }
}
