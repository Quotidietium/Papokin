use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 幽匿催发体催发时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SculkBloomEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub charge: i32,
}

impl SculkBloomEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, charge: i32) -> Self {
        Self {
            block_pos,
            world,
            charge,
            cancelled: false,
        }
    }
}
