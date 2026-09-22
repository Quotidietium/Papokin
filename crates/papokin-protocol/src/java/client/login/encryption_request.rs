use crate::ser::NetworkWriteExt;
use crate::{ClientPacket, MultiVersionJavaPacket};
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于发起加密握手。
///
/// 此数据包向客户端提供服务器的公钥以及
/// 验证令牌，使客户端能够生成共享密钥
/// 用于安全通信。
//#[java_packet(HELLO)]
pub struct CEncryptionRequest<'a> {
    /// 服务器的 ID 字符串。在现代 Minecraft 中，这通常是
    /// 一个空字符串，除非服务器正在使用旧版身份验证。
    pub server_id: &'a str,
    /// 服务器的 DER 编码 RSA 公钥。
    pub public_key: &'a [u8],
    /// 一个随机位串，用于验证客户端能否正确
    /// 使用服务器的公钥加密数据。
    pub verify_token: &'a [u8],
    /// 表示服务器是否处于“在线模式”并要求
    /// Mojang 身份验证。
    pub should_authenticate: bool,
}

impl MultiVersionJavaPacket for CEncryptionRequest<'_> {
    fn to_id(_version: JavaMinecraftVersion) -> i32 {
        1
    }
}

impl<'a> CEncryptionRequest<'a> {
    #[must_use]
    pub const fn new(
        server_id: &'a str,
        public_key: &'a [u8],
        verify_token: &'a [u8],
        should_authenticate: bool,
    ) -> Self {
        Self {
            server_id,
            public_key,
            verify_token,
            should_authenticate,
        }
    }
}

impl ClientPacket for CEncryptionRequest<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(self.server_id)?;
        if *version <= JavaMinecraftVersion::V_1_7_6 {
            write.write_i16_be(self.public_key.len() as i16)?;
        } else {
            write.write_var_int(&crate::VarInt(self.public_key.len() as i32))?;
        }
        write.write_all(self.public_key)?;
        if *version <= JavaMinecraftVersion::V_1_7_6 {
            write.write_i16_be(self.verify_token.len() as i16)?;
        } else {
            write.write_var_int(&crate::VarInt(self.verify_token.len() as i32))?;
        }
        write.write_all(self.verify_token)?;
        if version >= &JavaMinecraftVersion::V_1_20_5 {
            write.write_bool(self.should_authenticate)?;
        }
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for CEncryptionRequest<'a> {
    fn read(
        read: &mut &'a [u8],
        version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ReadingError> {
        use crate::ser::{NetworkReadExt, NetworkReadSliceExt};
        let server_id = read.get_str_bounded_borrowed(20)?;
        let public_key_len = if *version <= JavaMinecraftVersion::V_1_7_6 {
            let pkl = read.get_i16_be()?;
            if pkl < 0 {
                return Err(crate::ReadingError::Message(
                    "Key was smaller than nothing! Weird key!".into(),
                ));
            }
            pkl as usize
        } else {
            read.get_var_int()?.0 as usize
        };
        let public_key = read.read_slice_borrowed(public_key_len)?;

        let verify_token_len = if *version <= JavaMinecraftVersion::V_1_7_6 {
            let vtl = read.get_i16_be()?;
            if vtl < 0 {
                return Err(crate::ReadingError::Message(
                    "Key was smaller than nothing! Weird key!".into(),
                ));
            }
            vtl as usize
        } else {
            read.get_var_int()?.0 as usize
        };
        let verify_token = read.read_slice_borrowed(verify_token_len)?;

        let should_authenticate = if version >= &JavaMinecraftVersion::V_1_20_5 {
            read.get_bool()?
        } else {
            true
        };
        Ok(Self {
            server_id,
            public_key,
            verify_token,
            should_authenticate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServerPacket;

    #[test]
    fn encryption_request_roundtrip() {
        let packet =
            CEncryptionRequest::new("test_server", b"public_key_bytes", b"verify_1234", true);
        let mut buf = Vec::new();
        let version = JavaMinecraftVersion::V_1_21_4;
        packet.write_packet_data(&mut buf, &version).unwrap();

        let mut slice = buf.as_slice();
        let read_packet = CEncryptionRequest::read(&mut slice, &version).unwrap();
        assert_eq!(read_packet.server_id, packet.server_id);
        assert_eq!(read_packet.public_key, packet.public_key);
        assert_eq!(read_packet.verify_token, packet.verify_token);
        assert_eq!(read_packet.should_authenticate, packet.should_authenticate);
    }
}
