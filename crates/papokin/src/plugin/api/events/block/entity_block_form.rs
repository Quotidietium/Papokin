use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::EntityBase, world::World};

/// 因实体的行为而形成方块时发生的事件（例如冰霜行者、雪傀儡）。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityBlockFormEvent {
    pub entity: Arc<dyn EntityBase>,
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub new_state_id: BlockStateId,
}

impl EntityBlockFormEvent {
    #[must_use]
    pub const fn new(
        entity: Arc<dyn EntityBase>,
        block_pos: BlockPos,
        world: Arc<World>,
        new_state_id: BlockStateId,
    ) -> Self {
        Self {
            entity,
            block_pos,
            world,
            new_state_id,
            cancelled: false,
        }
    }
}
