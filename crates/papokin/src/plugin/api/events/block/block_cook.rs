use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 方块烹制物品时发生的事件（例如营火）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockCookEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub source: ItemStack,
    pub result: ItemStack,
}

impl BlockCookEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        source: ItemStack,
        result: ItemStack,
    ) -> Self {
        Self {
            block_pos,
            world,
            source,
            result,
            cancelled: false,
        }
    }
}
