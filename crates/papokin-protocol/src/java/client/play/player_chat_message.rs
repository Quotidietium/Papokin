use std::io::Write;

use papokin_data::packet::clientbound::play::PLAYER_CHAT;
use papokin_macros::java_packet;
use papokin_util::{text::TextComponent, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, WritingError,
    codec::{bit_set::BitSet, var_int::VarInt},
    ser::NetworkWriteExt,
};

/// 向客户端发送一条经过密码学签名的玩家聊天消息。
///
/// 此数据包是现代安全聊天系统的骨干。它包含
/// 跟踪索引、数字签名以及用于报告的上下文。
#[java_packet(PLAYER_CHAT)]
pub struct CPlayerChatMessage {
    /// 发送给此特定客户端的消息的递增索引。
    /// 登录时从 0 开始；若序列被打断，客户端会断开连接。
    pub global_index: VarInt,
    /// 发送消息的玩家的 UUID。
    pub sender: uuid::Uuid,
    /// 发送者玩家发出的消息的递增索引。
    /// 客户端用它来验证发送者历史记录的顺序。
    pub index: VarInt,
    /// 用于验证消息真实性的 RSA 签名（256 字节）。
    pub message_signature: Option<Box<[u8]>>,
    /// 消息的原始纯文本内容。
    pub message: Box<str>,
    /// 消息发送时的纪元时间戳（毫秒）。
    pub timestamp: i64,
    /// 一个用于确保签名唯一性的随机 64 位值。
    pub salt: i64,
    /// 发送者最近见到的 20 条消息签名，用于提供上下文
    /// 用于聊天举报，并确保没有遗漏任何消息。
    pub previous_messages: Box<[PreviousMessage]>,
    /// 消息的可选格式化版本（例如，如果服务器
    /// 添加了签名原始文本中不存在的颜色或链接）。
    pub unsigned_content: Option<TextComponent>,
    /// 表示消息是否应被隐藏或部分遮蔽
    /// 交由客户端的不雅用语过滤器处理。
    pub filter_type: FilterType,
    /// 聊天类型注册表条目的 ID（例如 "chat"、"`say_command`"）。
    /// 通常是 `(index + 1)`。
    pub chat_type: VarInt,
    /// 发送者的显示名称。
    pub sender_name: TextComponent,
    /// 目标的显示名称（用于私信）。
    pub target_name: Option<TextComponent>,
}

impl CPlayerChatMessage {
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        global_index: VarInt,
        sender: uuid::Uuid,
        index: VarInt,
        message_signature: Option<Box<[u8]>>,
        message: Box<str>,
        timestamp: i64,
        salt: i64,
        previous_messages: Box<[PreviousMessage]>,
        unsigned_content: Option<TextComponent>,
        filter_type: FilterType,
        chat_type: VarInt,
        sender_name: TextComponent,
        target_name: Option<TextComponent>,
    ) -> Self {
        Self {
            global_index,
            sender,
            index,
            message_signature,
            message,
            timestamp,
            salt,
            previous_messages,
            unsigned_content,
            filter_type,
            chat_type,
            sender_name,
            target_name,
        }
    }
}

//TODO: 检查我们是否需要这个自定义实现
impl ClientPacket for CPlayerChatMessage {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        write.write_var_int(&self.global_index)?;
        write.write_uuid(&self.sender)?;
        write.write_var_int(&self.index)?;
        write.write_option(&self.message_signature, |p, v| p.write_slice(v))?;
        write.write_string(&self.message)?;
        write.write_i64_be(self.timestamp)?;
        write.write_i64_be(self.salt)?;
        write.write_list(&self.previous_messages, |p, v| {
            p.write_var_int(&v.id)?;
            if let Some(signature) = &v.signature {
                p.write_slice(signature)?;
            }
            Ok(())
        })?;
        write.write_option(&self.unsigned_content, |p, v| p.write_component(v, version))?;
        match &self.filter_type {
            FilterType::PassThrough => write.write_var_int(&VarInt(0))?,
            FilterType::FullyFiltered => write.write_var_int(&VarInt(1))?,
            FilterType::PartiallyFiltered(bit_set) => {
                write.write_var_int(&VarInt(2))?;
                bit_set.encode_with_version(&mut write, version)?;
            }
        }
        write.write_var_int(&self.chat_type)?;
        write.write_component(&self.sender_name, version)?;
        write.write_option(&self.target_name, |p, v| p.write_component(v, version))?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct PreviousMessage {
    pub id: VarInt,
    pub signature: Option<Box<[u8]>>, // 恒为 256
}

pub enum FilterType {
    /// 消息完全不被过滤
    PassThrough,
    /// 消息被完全过滤
    FullyFiltered,
    /// 仅过滤消息中的部分字符
    PartiallyFiltered(BitSet),
}
