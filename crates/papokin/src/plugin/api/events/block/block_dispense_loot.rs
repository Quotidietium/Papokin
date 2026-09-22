use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 方块分发战利品时发生的事件（例如宝库、试炼刷怪笼）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockDispenseLootEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub items: Vec<ItemStack>,
}

impl BlockDispenseLootEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, items: Vec<ItemStack>) -> Self {
        Self {
            block_pos,
            world,
            items,
            cancelled: false,
        }
    }
}
