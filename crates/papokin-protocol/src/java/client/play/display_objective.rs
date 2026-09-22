use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError},
};
use papokin_data::{
    packet::clientbound::play::SET_DISPLAY_OBJECTIVE, scoreboard::ScoreboardDisplaySlot,
};
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 指示客户端在给定槽位中显示特定的记分板记分项。
///
/// 此数据包是向玩家展示记分板的最后一步。在此之后，
/// 一个记分项被创建并填入分数后，此数据包会“映射”
/// 将该记分目标显示到侧边栏或玩家列表等可视位置。
#[derive(Debug, PartialEq, Eq, Clone)]
#[java_packet(SET_DISPLAY_OBJECTIVE)]
pub struct CDisplayObjective {
    /// 记分板项的显示槽位/位置。
    pub position: VarInt,
    /// 要显示的记分板目标的唯一内部名称。
    /// 要在特定槽位中隐藏某个记分板目标，请发送空字符串。
    pub score_name: String,
}

impl CDisplayObjective {
    #[must_use]
    pub const fn new(position: ScoreboardDisplaySlot, score_name: String) -> Self {
        Self {
            position: VarInt(position as i32),
            score_name,
        }
    }
}

impl ClientPacket for CDisplayObjective {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if *version >= JavaMinecraftVersion::V_1_20_2 {
            write.write_var_int(&self.position)?;
        } else {
            write.write_i8(self.position.0 as i8)?;
        }

        if *version >= JavaMinecraftVersion::V_1_18 {
            write.write_string(&self.score_name)?;
        } else {
            write.write_string_bounded(&self.score_name, 16)?;
        }
        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CDisplayObjective {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let position = if *version >= JavaMinecraftVersion::V_1_20_2 {
            bytebuf.get_var_int()?
        } else {
            VarInt(i32::from(bytebuf.get_i8()?))
        };

        let score_name = if *version >= JavaMinecraftVersion::V_1_18 {
            bytebuf.get_str()?.into()
        } else {
            bytebuf.get_str_bounded(16)?.into()
        };

        Ok(Self {
            position,
            score_name,
        })
    }
}
