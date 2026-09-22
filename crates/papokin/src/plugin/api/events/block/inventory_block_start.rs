use papokin_macros::Event;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 物品栏方块开始一次操作时触发的事件。
#[derive(Event, Clone)]
pub struct InventoryBlockStartEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
}

impl InventoryBlockStartEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>) -> Self {
        Self { block_pos, world }
    }
}
