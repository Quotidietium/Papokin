use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 钟共鸣并高亮附近袭击者时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BellResonateEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
}

impl BellResonateEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>) -> Self {
        Self {
            block_pos,
            world,
            cancelled: false,
        }
    }
}
