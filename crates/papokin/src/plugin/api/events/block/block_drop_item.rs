use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

/// 方块向世界掉落物品时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockDropItemEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub player: Option<Arc<Player>>,
    pub items: Vec<ItemStack>,
}

impl BlockDropItemEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        player: Option<Arc<Player>>,
        items: Vec<ItemStack>,
    ) -> Self {
        Self {
            block_pos,
            world,
            player,
            items,
            cancelled: false,
        }
    }
}
