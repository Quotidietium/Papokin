use papokin_data::packet::clientbound::play::SERVER_DATA;
use papokin_macros::java_packet;
use papokin_util::text::TextComponent;

use crate::{ClientPacket, VarInt, ser::NetworkWriteExt};
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(SERVER_DATA)]
pub struct CServerData<'a> {
    pub motd: &'a TextComponent,
    pub icon_base64: Option<&'a str>,
}

impl<'a> CServerData<'a> {
    #[must_use]
    pub const fn new(motd: &'a TextComponent, icon_base64: Option<&'a str>) -> Self {
        Self { motd, icon_base64 }
    }
}

impl ClientPacket for CServerData<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if *version >= JavaMinecraftVersion::V_1_19_4 {
            write.write_component(self.motd, version)?;
            if let Some(icon) = self.icon_base64 {
                write.write_bool(true)?;
                let raw_b64 = icon.strip_prefix("data:image/png;base64,").unwrap_or(icon);
                // 原版 FriendlyByteBuf.writeByteArray = VarInt 长度前缀 + 原始字节。
                // 缺前缀时客户端把图标首字节误读为长度，整包解码失败
                // （DecoderException: Failed to decode packet 'clientbound/minecraft:server_data'）。
                let bytes =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_b64)
                        .unwrap_or_default();
                write.write_var_int(&VarInt(bytes.len() as i32))?;
                write.write_slice(&bytes)?;
            } else {
                write.write_bool(false)?;
            }
        } else {
            write.write_bool(true)?;
            write.write_component(self.motd, version)?;
            if let Some(icon) = self.icon_base64 {
                write.write_bool(true)?;
                write.write_string(icon)?;
            } else {
                write.write_bool(false)?;
            }
            if *version < JavaMinecraftVersion::V_1_19_3 {
                write.write_bool(false)?;
            }
            if *version >= JavaMinecraftVersion::V_1_19_1
                && *version < JavaMinecraftVersion::V_1_20_5
            {
                write.write_bool(false)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialize(motd: &TextComponent, icon: Option<&str>) -> Vec<u8> {
        let packet = CServerData::new(motd, icon);
        let mut bytes = Vec::new();
        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_1_21_11)
            .unwrap();
        bytes
    }

    #[test]
    fn 有图标时写varint长度前缀() {
        // "hello" 的 base64 = aGVsbG8=（5 字节）；原版 writeByteArray 布局：
        // [bool true=1] [VarInt 长度=5] [原始字节]
        let motd = TextComponent::text("hi");
        let bytes = serialize(&motd, Some("data:image/png;base64,aGVsbG8="));
        assert!(
            bytes.ends_with(&[1, 5, b'h', b'e', b'l', b'l', b'o']),
            "图标应以 VarInt 长度前缀写出，实际尾部: {bytes:?}"
        );
    }

    #[test]
    fn 无data前缀的图标同样带长度前缀() {
        let motd = TextComponent::text("hi");
        let bytes = serialize(&motd, Some("aGVsbG8="));
        assert!(bytes.ends_with(&[1, 5, b'h', b'e', b'l', b'l', b'o']));
    }

    #[test]
    fn 无图标写false() {
        let motd = TextComponent::text("hi");
        let bytes = serialize(&motd, None);
        assert!(bytes.ends_with(&[0]));
    }

    #[test]
    fn 非法base64回退为空字节数组() {
        let motd = TextComponent::text("hi");
        let bytes = serialize(&motd, Some("!!!not-base64!!!"));
        assert!(bytes.ends_with(&[1, 0]));
    }
}
