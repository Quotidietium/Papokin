use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// 方块的破坏进度针对玩家更新时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockBreakProgressUpdateEvent {
    /// 破坏方块的玩家。
    pub player: Arc<Player>,

    /// 正在被破坏的方块的位置。
    pub block_pos: BlockPos,

    /// 当前破坏进度，范围 0.0 到 1.0，重置时为 -1.0。
    pub progress: f32,
}

impl BlockBreakProgressUpdateEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, progress: f32) -> Self {
        Self {
            player,
            block_pos,
            progress,
            cancelled: false,
        }
    }
}
