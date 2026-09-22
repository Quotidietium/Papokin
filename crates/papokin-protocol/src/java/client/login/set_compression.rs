use papokin_data::packet::clientbound::login::LOGIN_COMPRESSION;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于为后续所有数据包启用网络压缩。
///
/// 此数据包一旦发送，服务器和客户端就必须压缩任何
/// 大小达到或超过指定阈值的数据包。
#[java_packet(LOGIN_COMPRESSION)]
pub struct CSetCompression {
    /// 应用压缩的数据包大小阈值（以字节为单位）。
    ///
    /// 小于该值的数据包将以未压缩方式发送。负阈值
    /// 通常会禁用压缩。
    pub threshold: VarInt,
}

impl CSetCompression {
    #[must_use]
    pub const fn new(threshold: VarInt) -> Self {
        Self { threshold }
    }
}

impl ClientPacket for CSetCompression {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.threshold)?;
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for CSetCompression {
    fn read(
        read: &mut &'a [u8],
        _version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ReadingError> {
        use crate::ser::NetworkReadExt;
        Ok(Self {
            threshold: read.get_var_int()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServerPacket;

    #[test]
    fn set_compression_roundtrip() {
        let packet = CSetCompression::new(crate::VarInt(256));
        let mut buf = Vec::new();
        let version = JavaMinecraftVersion::V_1_21_4;
        packet.write_packet_data(&mut buf, &version).unwrap();

        let mut slice = buf.as_slice();
        let read_packet = CSetCompression::read(&mut slice, &version).unwrap();
        assert_eq!(read_packet.threshold.0, 256);
    }
}
