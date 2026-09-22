use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 自动合成器合成物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CrafterCraftEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub result: ItemStack,
}

impl CrafterCraftEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, result: ItemStack) -> Self {
        Self {
            block_pos,
            world,
            result,
            cancelled: false,
        }
    }
}
