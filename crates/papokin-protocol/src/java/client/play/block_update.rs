use papokin_data::block_state_remap::remap_block_state_for_version;
use papokin_data::packet::clientbound::play::BLOCK_UPDATE;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;

use papokin_macros::java_packet;
use std::io::Write;

use crate::{
    ClientPacket, VarInt,
    ser::{NetworkWriteExt, WritingError},
};

/// 更新世界中特定位置的单个方块状态。
///
/// 这是将世界更改同步到客户端最常用的方式，例如
/// 当玩家放置方块、流体流动或红石元件切换时。
#[java_packet(BLOCK_UPDATE)]
pub struct CBlockUpdate {
    /// 正在更新的方块的世界坐标。
    pub location: BlockPos,
    /// 新的方块状态 ID。
    pub state_id: VarInt,
}

impl CBlockUpdate {
    #[must_use]
    pub const fn new(location: BlockPos, state_id: VarInt) -> Self {
        Self { location, state_id }
    }
}

impl ClientPacket for CBlockUpdate {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;
        write.write_block_pos(&self.location, version)?;

        let remapped_state = u16::try_from(self.state_id.0).map_or(self.state_id.0, |state_id| {
            i32::from(remap_block_state_for_version(state_id, *version))
        });
        write.write_var_int(&VarInt(remapped_state))?;

        Ok(())
    }
}
