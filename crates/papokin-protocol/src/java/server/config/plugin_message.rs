use papokin_data::packet::serverbound::config::CUSTOM_PAYLOAD;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{ReadingError, ServerPacket, ser::NetworkReadSliceExt};

/// 插件消息负载允许的最大大小（1 MiB）。
const MAX_PAYLOAD_SIZE: usize = 1_048_576;

/// 一个用于客户端与服务器之间自定义通信的数据包。
///
/// 这允许模组、插件或代理
/// 软件经由标准 Minecraft 协议发送专有数据。
#[java_packet(CUSTOM_PAYLOAD)]
pub struct SPluginMessage<'a> {
    /// 用于区分不同类型消息的通道名称。
    /// 示例：`minecraft:brand` 或 `velocity:main`。
    pub channel: &'a str,
    /// 客户端发送的负载。
    pub data: &'a [u8],
}

impl<'a> ServerPacket<'a> for SPluginMessage<'a> {
    fn read(read: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            channel: read.get_str_borrowed()?,
            data: read.read_remaining_slice_borrowed(MAX_PAYLOAD_SIZE)?,
        })
    }
}

impl crate::ClientPacket for SPluginMessage<'_> {
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
