use papokin_data::packet::serverbound::play::CHAT_COMMAND;
use papokin_macros::java_packet;

use crate::{
    ServerPacket,
    ser::{NetworkReadSliceExt, ReadingError},
};
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(CHAT_COMMAND)]
pub struct SChatCommand<'a> {
    pub command: &'a str,
}

impl<'a> ServerPacket<'a> for SChatCommand<'a> {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            // 与原版一致：命令字符串上限 256 字符（签名命令包
            // CHAT_COMMAND_SIGNED 同限）。经此入口的 /teammsg 等
            // 广播型命令的消息长度随之受限，防止以 32KB 级超长
            // 命令向队伍/全服放大消息组件序列化。
            command: bytebuf.get_str_bounded_borrowed(256)?,
        })
    }
}

impl crate::ClientPacket for SChatCommand<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_string(self.command)?;
        Ok(())
    }
}
