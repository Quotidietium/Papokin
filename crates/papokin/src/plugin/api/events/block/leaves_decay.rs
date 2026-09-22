use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 树叶自然枯萎时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct LeavesDecayEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
}

impl LeavesDecayEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>) -> Self {
        Self {
            block_pos,
            world,
            cancelled: false,
        }
    }
}
