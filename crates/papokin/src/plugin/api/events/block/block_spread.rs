use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 方块根据世界条件蔓延到另一位置时发生的事件（例如草、菌丝体、火）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockSpreadEvent {
    pub source_pos: BlockPos,
    pub target_pos: BlockPos,
    pub world: Arc<World>,
    pub new_state_id: BlockStateId,
}

impl BlockSpreadEvent {
    #[must_use]
    pub const fn new(
        source_pos: BlockPos,
        target_pos: BlockPos,
        world: Arc<World>,
        new_state_id: BlockStateId,
    ) -> Self {
        Self {
            source_pos,
            target_pos,
            world,
            new_state_id,
            cancelled: false,
        }
    }
}
