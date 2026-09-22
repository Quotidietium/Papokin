use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// 宝库展示物品时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VaultDisplayItemEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub item: ItemStack,
}

impl VaultDisplayItemEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, item: ItemStack) -> Self {
        Self {
            block_pos,
            world,
            item,
            cancelled: false,
        }
    }
}
