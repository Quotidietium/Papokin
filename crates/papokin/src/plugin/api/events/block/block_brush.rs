use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

/// 玩家刷拭可刷拭方块（例如可疑的沙子/沙砾）时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockBrushEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub player: Arc<Player>,
    pub item: ItemStack,
}

impl BlockBrushEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        player: Arc<Player>,
        item: ItemStack,
    ) -> Self {
        Self {
            block_pos,
            world,
            player,
            item,
            cancelled: false,
        }
    }
}
