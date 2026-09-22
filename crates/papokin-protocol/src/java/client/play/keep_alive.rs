use papokin_data::packet::clientbound::play::KEEP_ALIVE;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 维持连接并测量客户端与服务器之间的延迟（ping）。
///
/// 服务器定期发送此数据包（通常每 15–20 秒一次）。
/// 客户端必须以完全相同的 ID 响应。如果服务器没有收到
/// 超时时间（通常为 30 秒）内的响应，它就会断开连接
/// 玩家并附上 "Timed Out"（连接超时）消息。
#[java_packet(KEEP_ALIVE)]
pub struct CKeepAlive {
    /// 此特定 keep-alive 请求的唯一随机标识符。
    /// 用于将服务器的请求与客户端的响应相匹配。
    pub keep_alive_id: i64,
}

impl CKeepAlive {
    #[must_use]
    pub const fn new(keep_alive_id: i64) -> Self {
        Self { keep_alive_id }
    }
}

impl ClientPacket for CKeepAlive {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if version >= &JavaMinecraftVersion::V_1_12_2 {
            write.write_i64_be(self.keep_alive_id)?;
        } else if version >= &JavaMinecraftVersion::V_1_8 {
            write.write_var_int(&VarInt(self.keep_alive_id as i32))?;
        } else {
            write.write_i32_be(self.keep_alive_id as i32)?;
        }
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for CKeepAlive {
    fn read(
        read: &mut &'a [u8],
        version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ReadingError> {
        use crate::ser::NetworkReadExt;
        let keep_alive_id = if version >= &JavaMinecraftVersion::V_1_12_2 {
            read.get_i64_be()?
        } else if version >= &JavaMinecraftVersion::V_1_8 {
            i64::from(read.get_var_int()?.0)
        } else {
            i64::from(read.get_i32_be()?)
        };
        Ok(Self { keep_alive_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServerPacket;

    #[test]
    fn keep_alive_roundtrip_modern() {
        let packet = CKeepAlive::new(1234567890123456789);
        let mut buf = Vec::new();
        let version = JavaMinecraftVersion::V_1_21_4;
        packet.write_packet_data(&mut buf, &version).unwrap();

        let mut slice = buf.as_slice();
        let read_packet = CKeepAlive::read(&mut slice, &version).unwrap();
        assert_eq!(read_packet.keep_alive_id, 1234567890123456789);
    }

    #[test]
    fn keep_alive_roundtrip_1_8() {
        let packet = CKeepAlive::new(12345);
        let mut buf = Vec::new();
        let version = JavaMinecraftVersion::V_1_8;
        packet.write_packet_data(&mut buf, &version).unwrap();

        let mut slice = buf.as_slice();
        let read_packet = CKeepAlive::read(&mut slice, &version).unwrap();
        assert_eq!(read_packet.keep_alive_id, 12345);
    }

    #[test]
    fn keep_alive_roundtrip_1_7() {
        let packet = CKeepAlive::new(12345);
        let mut buf = Vec::new();
        let version = JavaMinecraftVersion::V_1_7_2;
        packet.write_packet_data(&mut buf, &version).unwrap();

        let mut slice = buf.as_slice();
        let read_packet = CKeepAlive::read(&mut slice, &version).unwrap();
        assert_eq!(read_packet.keep_alive_id, 12345);
    }
}
