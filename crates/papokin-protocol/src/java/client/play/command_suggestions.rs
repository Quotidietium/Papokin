use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::COMMAND_SUGGESTIONS;
use papokin_macros::java_packet;
use papokin_util::text::TextComponent;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于提供“Tab 补全”建议列表。
///
/// 此数据包响应客户端对命令帮助的请求，
/// 在玩家输入时以可滚动的选项列表形式出现
/// 在聊天栏中。
#[java_packet(COMMAND_SUGGESTIONS)]
pub struct CCommandSuggestions {
    /// 此响应所针对的请求的唯一 ID。
    /// 此值必须与客户端在请求数据包中发送的 ID 匹配。
    pub id: VarInt,
    /// 聊天栏中补全开始处的字符索引
    /// 应插入的位置。
    pub start: VarInt,
    /// 原文本中要替换掉的字符数量
    /// 该建议。
    pub length: VarInt,
    /// 可能的补全列表，其中可包含工具提示
    /// 以获取额外上下文。
    pub matches: Box<[CommandSuggestion]>,
}

impl CCommandSuggestions {
    #[must_use]
    pub const fn new(
        id: VarInt,
        start: VarInt,
        length: VarInt,
        matches: Box<[CommandSuggestion]>,
    ) -> Self {
        Self {
            id,
            start,
            length,
            matches,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct CommandSuggestion {
    pub suggestion: String,
    pub tooltip: Option<TextComponent>,
}

impl CommandSuggestion {
    #[must_use]
    pub const fn new(suggestion: String, tooltip: Option<TextComponent>) -> Self {
        Self {
            suggestion,
            tooltip,
        }
    }

    pub fn write(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(&self.suggestion)?;
        if let Some(tooltip) = &self.tooltip {
            write.write_bool(true)?;
            write.write_component(tooltip, version)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}

impl ClientPacket for CCommandSuggestions {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if *version >= JavaMinecraftVersion::V_1_13 {
            write.write_var_int(&self.id)?;
            write.write_var_int(&self.start)?;
            write.write_var_int(&self.length)?;
            write.write_var_int(&VarInt(self.matches.len() as i32))?;
            for match_ in &self.matches {
                match_.write(&mut write, version)?;
            }
        } else {
            write.write_var_int(&VarInt(self.matches.len() as i32))?;
            for match_ in &self.matches {
                write.write_string(&match_.suggestion)?;
            }
        }
        Ok(())
    }
}
