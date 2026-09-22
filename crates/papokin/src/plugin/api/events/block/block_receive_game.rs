use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::EntityBase, world::World};

/// 方块接收到游戏事件时发生的事件（例如幽匿感测体）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockReceiveGameEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub game_event: String,
    pub source_entity: Option<Arc<dyn EntityBase>>,
}

impl BlockReceiveGameEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        game_event: String,
        source_entity: Option<Arc<dyn EntityBase>>,
    ) -> Self {
        Self {
            block_pos,
            world,
            game_event,
            source_entity,
            cancelled: false,
        }
    }
}
