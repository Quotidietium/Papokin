use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError},
};
use papokin_data::packet::clientbound::play::CHANGE_DIFFICULTY;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 通知客户端世界的难度等级或锁定状态已更改。
///
/// 此数据包更新客户端的内部状态，会影响某些 UI 元素
/// 以及客户端侧行为（不过生物伤害等实际游戏逻辑
/// 主要由服务器处理）。
#[java_packet(CHANGE_DIFFICULTY)]
pub struct CChangeDifficulty {
    /// 世界当前的游戏难度。
    ///
    /// * **0**: 和平
    /// * **1**: 简单
    /// * **2**: 普通
    /// * **3**: 困难
    pub difficulty: u8,
    /// 难度是否被锁定（1.14 新增）。若为 true，客户端的难度
    /// 选项菜单中的开关将被禁用。
    pub locked: bool,
}

impl CChangeDifficulty {
    #[must_use]
    pub const fn new(difficulty: u8, locked: bool) -> Self {
        Self { difficulty, locked }
    }
}

impl ClientPacket for CChangeDifficulty {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        // 难度枚举在 1.21.6+ 序列化为 VarInt，更早之前为无符号字节
        if *version >= JavaMinecraftVersion::V_1_21_6 {
            write.write_var_int(&VarInt(i32::from(self.difficulty)))?;
        } else {
            write.write_u8(self.difficulty)?;
        }
        // 1.14 新增：locked 布尔值
        if *version >= JavaMinecraftVersion::V_1_14 {
            write.write_bool(self.locked)?;
        }
        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CChangeDifficulty {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let difficulty = if *version >= JavaMinecraftVersion::V_1_21_6 {
            bytebuf.get_var_int()?.0 as u8
        } else {
            bytebuf.get_u8()?
        };
        let locked = if *version >= JavaMinecraftVersion::V_1_14 {
            bytebuf.get_bool()?
        } else {
            false
        };
        Ok(Self { difficulty, locked })
    }
}
