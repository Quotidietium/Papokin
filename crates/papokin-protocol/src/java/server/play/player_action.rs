use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use papokin_data::packet::serverbound::play::PLAYER_ACTION;
use papokin_macros::java_packet;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;

use crate::VarInt;

#[java_packet(PLAYER_ACTION)]
pub struct SPlayerAction {
    pub status: VarInt,
    pub position: BlockPos,
    pub face: u8,
    pub sequence: VarInt,
}

impl<'a> ServerPacket<'a> for SPlayerAction {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let status = if version >= &JavaMinecraftVersion::V_1_9 {
            bytebuf.get_var_int()?
        } else {
            VarInt(i32::from(bytebuf.get_u8()?))
        };
        let status = status_from_version(status, *version);
        let position = bytebuf.get_block_pos(version)?;
        let face = bytebuf.get_u8()?;
        let sequence = if version >= &JavaMinecraftVersion::V_1_19 {
            bytebuf.get_var_int()?
        } else {
            VarInt(0)
        };

        Ok(Self {
            status,
            position,
            face,
            sequence,
        })
    }
}

impl crate::ClientPacket for SPlayerAction {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        let status = status_to_version(self.status, *version);
        if version >= &JavaMinecraftVersion::V_1_9 {
            write.write_var_int(&status)?;
        } else {
            write.write_u8(status.0 as u8)?;
        }
        write.write_block_pos(&self.position, version)?;
        write.write_u8(self.face)?;
        if version >= &JavaMinecraftVersion::V_1_19 {
            write.write_var_int(&self.sequence)?;
        }
        Ok(())
    }
}

/// 26.3 将“更改破坏方向”添加为动作 1，其后所有动作依次后移一位。我们
/// 内部沿用旧编号，并把最后一个 id 之后的新编号分配给该动作。
fn status_from_version(status: VarInt, version: JavaMinecraftVersion) -> VarInt {
    if version < JavaMinecraftVersion::V_26_3 || status.0 < 1 {
        return status;
    }
    if status.0 == 1 {
        VarInt(Status::ChangeDestroyDirection as i32)
    } else {
        VarInt(status.0 - 1)
    }
}

fn status_to_version(status: VarInt, version: JavaMinecraftVersion) -> VarInt {
    if version < JavaMinecraftVersion::V_26_3 || status.0 < 1 {
        return status;
    }
    if status.0 == Status::ChangeDestroyDirection as i32 {
        VarInt(1)
    } else {
        VarInt(status.0 + 1)
    }
}

pub enum Status {
    /// 当玩家开始挖掘方块时发送。如果方块被瞬间挖除或玩家处于创造模式，客户端将不会发送 `Status` = `FinishedDigging`，而是假定服务器已完成破坏。要检测这种情况，必须在服务端计算方块破坏速度。
    StartedDigging = 0,
    /// 当玩家松开挖掘方块键（默认：左键）时发送。面始终被设为 -Y。
    CancelledDigging,
    /// 当客户端认为自己已完成时发送。
    FinishedDigging,
    /// 由按下丢弃物品键（默认：Q）并配合丢弃整组所选物品的修饰键（默认：Control 或 Command，取决于操作系统）触发。位置始终设为 0/0/0，朝向始终设为 -Y，序列号始终设为 0。
    DropItemStack,
    /// 由按下丢弃物品键（默认：Q）触发。位置始终设为 0/0/0，朝向始终设为 -Y，序列号始终设为 0。
    DropItem,
    /// 这可不是我编的
    /// 表示当前手持物品的状态应被更新，例如进食、拉弓、使用桶等。位置始终设为 0/0/0，面始终设为 -Y，序列号始终设为 0。
    ReleaseItemInUse,
    /// 用于将物品换到或分配到副手。位置始终设为 0/0/0，面始终设为 -Y，序列始终设为 0。
    SwapItem,
    /// 当玩家手持矛并进行突刺攻击时发送。
    SpearJab,
    /// 自 26.3 起新增：当玩家继续挖掘但看向方块的另一面时发送。
    ChangeDestroyDirection,
}

pub struct InvalidStatus;

impl TryFrom<i32> for Status {
    type Error = InvalidStatus;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::StartedDigging),
            1 => Ok(Self::CancelledDigging),
            2 => Ok(Self::FinishedDigging),
            3 => Ok(Self::DropItemStack),
            4 => Ok(Self::DropItem),
            5 => Ok(Self::ReleaseItemInUse),
            6 => Ok(Self::SwapItem),
            7 => Ok(Self::SpearJab),
            8 => Ok(Self::ChangeDestroyDirection),
            _ => Err(InvalidStatus),
        }
    }
}
