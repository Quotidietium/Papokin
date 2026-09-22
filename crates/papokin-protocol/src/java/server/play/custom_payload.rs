use papokin_data::packet::serverbound::play::CUSTOM_PAYLOAD;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{ReadingError, ServerPacket, ser::NetworkReadSliceExt};

/// 游戏阶段自定义负载允许的最大大小（32 KiB）。
const MAX_PAYLOAD_SIZE: usize = 32_767;

/// 一个用于客户端与服务器之间自定义通信的数据包。
///
/// 这允许模组、插件或代理软件通过标准（协议）发送专有数据
/// Minecraft 协议。
#[java_packet(CUSTOM_PAYLOAD)]
pub struct SCustomPayload<'a> {
    /// 用于区分不同类型消息的通道名称。
    /// 示例：`minecraft:brand` 或 `voicechat:request_secret`。
    pub channel: &'a str,
    /// 客户端发送的负载。
    pub data: &'a [u8],
}

impl<'a> ServerPacket<'a> for SCustomPayload<'a> {
    fn read(read: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            channel: read.get_str_borrowed()?,
            data: read.read_remaining_slice_borrowed(MAX_PAYLOAD_SIZE)?,
        })
    }
}

impl crate::ClientPacket for SCustomPayload<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_string(self.channel)?;
        write.write_slice(self.data)?;
        Ok(())
    }
}
