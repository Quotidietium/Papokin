use bytes::{BufMut, BytesMut};
use thiserror::Error;

/// 客户端 -> 服务器
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerboundPacket {
    /// 通常是客户端发送的第一个数据包，用于对与服务器的连接进行认证。
    Auth = 3,
    /// 此数据包类型表示客户端向服务器发出的命令。可以是 `ConCommand`，例如 `/kill <player>` 或 `/weather clear`。
    /// 响应内容视所执行的命令而定。
    ExecCommand = 2,
}

impl ServerboundPacket {
    #[must_use]
    pub const fn from_i32(n: i32) -> Option<Self> {
        match n {
            3 => Some(Self::Auth),
            2 => Some(Self::ExecCommand),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 服务器 -> 客户端
pub enum ClientboundPacket {
    /// 此数据包是连接当前认证状态的通知。
    AuthResponse = 2,
    /// `SERVERDATA_RESPONSE` 数据包是对 `SERVERDATA_EXECCOMMAND` 请求的响应。
    Output = 0,
}

impl ClientboundPacket {
    #[must_use]
    pub fn write_buf(self, id: i32, body: &str) -> BytesMut {
        let mut buf = BytesMut::new();
        // 10 是 4 字节 ty、4 字节 id 和 2 个结尾空字节的和。
        buf.put_i32_le(10 + body.len() as i32);
        buf.put_i32_le(id);
        buf.put_i32_le(self as i32);
        let bytes = body.as_bytes();
        buf.put_slice(bytes);
        buf.put_u8(0);
        buf.put_u8(0);
        buf
    }
}

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("Invalid length")]
    InvalidLength,
    #[error("Failed to send packet: {0}")]
    FailedSend(std::io::Error),
    #[error("Missing packet null terminator")]
    MissingNullTerminator,
    #[error("Invalid packet string body: {0}")]
    InvalidBody(std::str::Utf8Error),
    #[error("Unknown packet type: {0}")]
    UnknownPacketType(i32),
}

#[derive(Debug, PartialEq, Eq)]
/// 发往服务器的数据包
pub struct Packet {
    id: i32,
    ptype: ServerboundPacket,
    body: Box<str>,
}

impl Packet {
    pub fn deserialize(incoming: &mut Vec<u8>) -> Result<Option<Self>, PacketError> {
        // 读取数据包长度头至少需要 4 个字节
        if incoming.len() < 4 {
            return Ok(None);
        }

        // 读取 4 字节长度头（小端序）
        let len_bytes: [u8; 4] = incoming[0..4]
            .try_into()
            .map_err(|_| PacketError::InvalidLength)?;
        let size = i32::from_le_bytes(len_bytes);

        // RCON 数据包大小包含：ID(4) + Type(4) + Body(n) + Null(1) + EmptyStringNull(1)。
        // 空主体的最小大小为 10。MTU 最大大小通常为 1460。
        if !(10..=1460).contains(&size) {
            return Err(PacketError::InvalidLength);
        }

        // 缓冲区中需要的总字节数（长度头 + 负载大小）
        let total_packet_len = (size as usize) + 4;

        // 如果网络尚未送达完整数据包，返回 Ok(None)
        if incoming.len() < total_packet_len {
            return Ok(None);
        }

        // 我们已拿到完整的数据包，用固定切片同步解析它。
        let id = i32::from_le_bytes(
            incoming[4..8]
                .try_into()
                .map_err(|_| PacketError::InvalidLength)?,
        );
        let ty = i32::from_le_bytes(
            incoming[8..12]
                .try_into()
                .map_err(|_| PacketError::InvalidLength)?,
        );

        let ptype = ServerboundPacket::from_i32(ty).ok_or(PacketError::UnknownPacketType(ty))?;

        // 计算主体边界（在 len、id 和 ty 之后开始 -> 4+4+4 = 12）
        // 在总长度前 2 字节处结束（不含末尾的两个空字节）
        let body_start = 12;
        let body_end = total_packet_len - 2;

        if incoming[body_end] != 0 || incoming[body_end + 1] != 0 {
            return Err(PacketError::MissingNullTerminator);
        }

        let payload = &incoming[body_start..body_end];
        let body = std::str::from_utf8(payload)
            .map_err(PacketError::InvalidBody)?
            .into();
        incoming.drain(0..total_packet_len);

        Ok(Some(Self { id, ptype, body }))
    }

    #[must_use]
    pub fn get_body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub const fn get_type(&self) -> ServerboundPacket {
        self.ptype
    }

    #[must_use]
    pub const fn get_id(&self) -> i32 {
        self.id
    }
}
