use papokin_data::packet::clientbound::play::{CHAT, SYSTEM_CHAT};
use papokin_util::text::TextComponent;
use papokin_util::version::JavaMinecraftVersion;

use crate::ClientPacket;
use crate::codec::var_int::VarInt;
use crate::packet::MultiVersionJavaPacket;
use crate::ser::NetworkWriteExt;

/// 向客户端发送系统聊天消息。
///
/// 系统消息是服务器自身发送的消息（例如加入/退出通知，
/// 命令反馈、服务器公告或 actionbar 覆盖消息）。
pub struct CSystemChatMessage<'a> {
    pub content: &'a TextComponent,
    /// 为 true 时，消息显示在快捷栏上方（动作栏）。
    /// 为 false 时，消息显示在普通聊天框中。
    pub overlay: bool,
}

impl<'a> CSystemChatMessage<'a> {
    #[must_use]
    pub const fn new(content: &'a TextComponent, overlay: bool) -> Self {
        Self { content, overlay }
    }
}

impl MultiVersionJavaPacket for CSystemChatMessage<'_> {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_1_19 {
            SYSTEM_CHAT.to_id(version)
        } else {
            CHAT.to_id(version)
        }
    }
}

impl ClientPacket for CSystemChatMessage<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.content, version)?;

        if *version >= JavaMinecraftVersion::V_1_19_1 {
            write.write_bool(self.overlay)?;
        } else if *version >= JavaMinecraftVersion::V_1_19 {
            // 在 1.19.0 中，类型 ID 是 VarInt：1 表示 SYSTEM，2 表示 GAME_INFO（actionbar/overlay）
            let type_id = if self.overlay { 2 } else { 1 };
            write.write_var_int(&VarInt(type_id))?;
        } else if *version >= JavaMinecraftVersion::V_1_16 {
            // 在 1.16 - 1.18.2 中：位置字节（1 表示系统，2 表示 game_info）+ 发送者 UUID
            let position = if self.overlay { 2 } else { 1 };
            write.write_u8(position)?;
            write.write_uuid(&uuid::Uuid::nil())?;
        } else if *version >= JavaMinecraftVersion::V_1_8 {
            // 在 1.8 - 1.15.2 中：位置字节（1 表示系统，2 表示 game_info）
            let position = if self.overlay { 2 } else { 1 };
            write.write_u8(position)?;
        }
        // 在 1.7.2 - 1.7.10 中：聊天数据包中只有 component

        Ok(())
    }
}
