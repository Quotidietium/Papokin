use crate::{
    ClientPacket, ConnectionState, ReadingError, ServerPacket, VarInt, ser::NetworkReadExt,
    ser::NetworkWriteExt,
};
use papokin_data::packet::serverbound::handshake::INTENTION;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 客户端为发起连接而发送的第一个数据包
///
/// 它判断客户端是否想查询服务器状态（SLP）
/// 还是真正登录进行游戏。
#[java_packet(INTENTION)]
pub struct SHandShake {
    /// 客户端的协议版本（例如 1.21 对应 767）。
    pub protocol_version: VarInt,
    /// 客户端用于连接的主机名或 IP
    pub server_address: Box<str>,
    /// 客户端用于连接的端口号
    pub server_port: u16,
    /// 客户端想要切换到的状态（1 为 Status，2 为 Login）
    pub next_state: ConnectionState,
}

impl<'a> ServerPacket<'a> for SHandShake {
    fn read(read: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            protocol_version: read.get_var_int()?,
            // 原版客户端在此处从不超过 255 个字符，但
            // `BungeeCord` 风格的 IP 转发会追加客户端的 IP、其 UUID
            // 以及其带签名的档案属性写入此字段，这样很容易
            // 超出该上限。读取时应采用 `write_string` 已使用的相同边界
            // 写入它时所用的值，就像 Spigot 那样。
            server_address: read.get_str_bounded(i16::MAX as usize)?,
            server_port: read.get_u16_be()?,
            next_state: read
                .get_var_int()?
                .try_into()
                .map_err(|_| ReadingError::Message("Invalid status".to_string()))?,
        })
    }
}

impl ClientPacket for SHandShake {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.protocol_version)?;
        write.write_string(&self.server_address)?;
        write.write_u16_be(self.server_port)?;
        write.write_var_int(&VarInt(self.next_state as i32))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按客户端或代理的发送方式编码握手包的正文。
    fn encode_handshake(server_address: &str, next_state: i32) -> Vec<u8> {
        let mut buf = Vec::new();
        let protocol_version = JavaMinecraftVersion::V_1_21_11.protocol_version();
        buf.write_var_int(&VarInt(protocol_version))
            .expect("写入协议版本");
        // 以输入本身为界，以便此辅助函数也能构建
        // 由 `rejects_oversized_server_address` 使用的超长地址。
        buf.write_string_bounded(server_address, server_address.len())
            .expect("写入服务器地址");
        buf.write_u16_be(25565).expect("写入服务器端口");
        buf.write_var_int(&VarInt(next_state))
            .expect("写入下一个状态");
        buf
    }

    /// 启用 `ip_forward` 时 `BungeeCord` 向下游发送的地址：
    /// 宿主、客户端 IP、其 UUID 以及已签名的档案属性，
    /// 以 NUL 字节分隔。`papokin` crate 中的 `bungeecord_login` 会拆分
    /// 将该精确值重新拆分回那四个部分。
    fn bungeecord_forwarded_address() -> String {
        let textures = "e".repeat(432);
        let signature = "s".repeat(684);
        format!(
            "mc.example.com\0192.0.2.10\0d8f4a1e0-0f1b-4c3a-9f2e-1a2b3c4d5e6f\0\
             [{{\"name\":\"textures\",\"value\":\"{textures}\",\"signature\":\"{signature}\"}}]"
        )
    }

    #[test]
    fn reads_bungeecord_forwarded_server_address() {
        let address = bungeecord_forwarded_address();
        let buf = encode_handshake(&address, 2);

        let packet = SHandShake::read(&mut &buf[..], &JavaMinecraftVersion::V_1_21_11)
            .expect("BungeeCord 转发的地址应当可读");

        assert_eq!(&*packet.server_address, address.as_str());
        assert_eq!(packet.server_address.split('\0').count(), 4);
        assert_eq!(packet.next_state, ConnectionState::Login);
    }

    #[test]
    fn reads_plain_server_address() {
        let buf = encode_handshake("localhost", 1);

        let packet = SHandShake::read(&mut &buf[..], &JavaMinecraftVersion::V_1_21_11)
            .expect("普通地址应当可读");

        assert_eq!(&*packet.server_address, "localhost");
        assert_eq!(packet.next_state, ConnectionState::Status);
    }

    #[test]
    fn rejects_oversized_server_address() {
        let address = "a".repeat(i16::MAX as usize + 1);
        let buf = encode_handshake(&address, 2);

        let result = SHandShake::read(&mut &buf[..], &JavaMinecraftVersion::V_1_21_11);

        assert!(matches!(result, Err(ReadingError::TooLarge(_))));
    }
}
