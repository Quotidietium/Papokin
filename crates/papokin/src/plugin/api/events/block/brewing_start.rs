use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 酿造台开始酿造时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BrewingStartEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub brewing_time: i32,
}

impl BrewingStartEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, brewing_time: i32) -> Self {
        Self {
            block_pos,
            world,
            brewing_time,
            cancelled: false,
        }
    }
}
