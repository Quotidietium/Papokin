use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::EntityBase, world::World};

/// 发射器为实体装备盔甲时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockDispenseArmorEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub target: Arc<dyn EntityBase>,
    pub item: ItemStack,
}

impl BlockDispenseArmorEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        target: Arc<dyn EntityBase>,
        item: ItemStack,
    ) -> Self {
        Self {
            block_pos,
            world,
            target,
            item,
            cancelled: false,
        }
    }
}
