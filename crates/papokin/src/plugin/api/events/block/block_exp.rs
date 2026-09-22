use papokin_macros::Event;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 方块产出经验时发生的事件（例如挖掘矿石）。
#[derive(Event, Clone)]
pub struct BlockExpEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub exp: i32,
}

impl BlockExpEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, exp: i32) -> Self {
        Self {
            block_pos,
            world,
            exp,
        }
    }
}
