use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 农田湿度等级变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct MoistureChangeEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub new_moisture: i32,
}

impl MoistureChangeEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, new_moisture: i32) -> Self {
        Self {
            block_pos,
            world,
            new_moisture,
            cancelled: false,
        }
    }
}
