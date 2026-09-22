use papokin_data::packet::clientbound::play::DELETE_CHAT;
use papokin_macros::java_packet;

use crate::{ClientPacket, codec::var_int::VarInt, ser::NetworkWriteExt};
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(DELETE_CHAT)]
pub struct CDeleteChat<'a> {
    pub signature_id: VarInt,
    pub signature: Option<&'a [u8]>,
}

impl<'a> CDeleteChat<'a> {
    /// 使用原始签名 ID `VarInt` 创建新的 `CDeleteChat` 数据包。
    #[must_use]
    pub const fn from_id(signature_id: VarInt) -> Self {
        Self {
            signature_id,
            signature: None,
        }
    }

    /// 使用以 0 为起始索引的签名缓存 ID 创建新的 `CDeleteChat` 数据包。
    ///
    /// 在 Minecraft 的 `MessageSignature.Packed` 中，缓存签名写作 `cache_id + 1`。
    #[must_use]
    pub const fn from_cache_id(cache_id: i32) -> Self {
        Self {
            signature_id: VarInt(cache_id + 1),
            signature: None,
        }
    }

    /// 使用完整的 256 字节消息签名创建新的 `CDeleteChat` 数据包。
    ///
    /// 在 Minecraft 的 `MessageSignature.Packed` 中，完整签名写作 `VarInt(0)`
    /// 之后是 256 个原始签名字节。
    #[must_use]
    pub const fn from_signature(signature: &'a [u8]) -> Self {
        Self {
            signature_id: VarInt(0),
            signature: Some(signature),
        }
    }
}

impl ClientPacket for CDeleteChat<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.signature_id)?;
        if let Some(signature) = self.signature {
            write.write_slice(signature)?;
        }
        Ok(())
    }
}
