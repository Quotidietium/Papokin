use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 流体液位变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct FluidLevelChangeEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub new_state_id: BlockStateId,
}

impl FluidLevelChangeEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, new_state_id: BlockStateId) -> Self {
        Self {
            block_pos,
            world,
            new_state_id,
            cancelled: false,
        }
    }
}
