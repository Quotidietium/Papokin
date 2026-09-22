use papokin_data::packet::clientbound::play::DISGUISED_CHAT;
use papokin_macros::java_packet;
use papokin_util::text::TextComponent;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 发送一条未经玩家密码学签名的聊天消息。
///
/// 引入它是为了支持服务器端的“伪装”身份（如 /say 或 NPC 聊天），
/// 此数据包绕过玩家间聊天签名要求，同时
/// 同时仍允许客户端用标准聊天注册表来格式化消息。
#[java_packet(DISGUISED_CHAT)]
pub struct CDisguisedChatMessage<'a> {
    /// 消息的原始内容。
    pub message: &'a TextComponent,
    /// `minecraft:chat_type` 注册表中的索引。
    /// 这决定装饰格式（例如 "<%s> %s" 或 "[%s -> %s] %s"）。
    pub chat_type: VarInt,
    /// 显示为消息“发送者”的名称。
    pub sender_name: &'a TextComponent,
    /// 可选名称，显示为“目标”（用于私信/私聊）。
    pub target_name: Option<&'a TextComponent>,
}

impl<'a> CDisguisedChatMessage<'a> {
    #[must_use]
    pub const fn new(
        message: &'a TextComponent,
        chat_type: VarInt,
        sender_name: &'a TextComponent,
        target_name: Option<&'a TextComponent>,
    ) -> Self {
        Self {
            message,
            chat_type,
            sender_name,
            target_name,
        }
    }
}

impl ClientPacket for CDisguisedChatMessage<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.message, version)?;
        write.write_var_int(&self.chat_type)?;
        write.write_component(self.sender_name, version)?;
        if let Some(target) = self.target_name {
            write.write_bool(true)?;
            write.write_component(target, version)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}
