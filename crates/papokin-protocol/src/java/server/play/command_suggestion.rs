use papokin_data::packet::serverbound::play::{COMMAND_SUGGESTION, COMMAND_SUGGESTIONS};

use crate::VarInt;

use crate::{
    ServerPacket,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};
use papokin_util::version::JavaMinecraftVersion;

pub struct SCommandSuggestion<'a> {
    pub id: VarInt,
    pub command: &'a str,
}

/// 26.2 将 `command_suggestions`（命令补全请求）更名为 `command_suggestion`，
/// 生成表中的两个常量各只覆盖一个时代，须按版本选择，
/// 否则 1.x 版本解析出的 id 为 -1，永远匹配不到分发臂。
impl crate::packet::MultiVersionJavaPacket for SCommandSuggestion<'_> {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_26_2 {
            COMMAND_SUGGESTION.to_id(version)
        } else {
            COMMAND_SUGGESTIONS.to_id(version)
        }
    }
}

impl<'a> ServerPacket<'a> for SCommandSuggestion<'a> {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        if *version >= JavaMinecraftVersion::V_1_13 {
            Ok(Self {
                id: bytebuf.get_var_int()?,
                command: bytebuf.get_str_borrowed()?,
            })
        } else {
            let command = bytebuf.get_str_borrowed()?;
            if *version >= JavaMinecraftVersion::V_1_9 {
                let _assume_command = bytebuf.get_bool()?;
            }
            if *version >= JavaMinecraftVersion::V_1_8 {
                let has_pos = bytebuf.get_bool()?;
                if has_pos {
                    let _ = bytebuf.get_block_pos(version)?;
                }
            }
            Ok(Self {
                id: VarInt(0),
                command,
            })
        }
    }
}

impl crate::ClientPacket for SCommandSuggestion<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_var_int(&self.id)?;
        write.write_string(self.command)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::MultiVersionJavaPacket;

    // 数值钉死为 assets/packet/<版本>_packets.json 的权威 id：
    // 26.2 更名后若时代选择回退（如误用单一常量），1.x 下会得到 -1
    #[test]
    fn id_matches_authoritative_table_per_era() {
        assert_eq!(
            SCommandSuggestion::to_id(JavaMinecraftVersion::V_1_21_11),
            14
        );
        assert_eq!(
            SCommandSuggestion::to_id(JavaMinecraftVersion::V_1_21_4),
            13
        );
        assert_eq!(SCommandSuggestion::to_id(JavaMinecraftVersion::V_26_1), 15);
        assert_eq!(SCommandSuggestion::to_id(JavaMinecraftVersion::V_26_2), 15);
        assert_eq!(SCommandSuggestion::to_id(JavaMinecraftVersion::V_26_3), 15);
    }
}
