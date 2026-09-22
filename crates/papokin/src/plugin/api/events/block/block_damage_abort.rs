use papokin_data::item_stack::ItemStack;
use papokin_macros::Event;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

/// 玩家在方块被破坏之前停止伤害/挖掘时发生的事件。
#[derive(Event, Clone)]
pub struct BlockDamageAbortEvent {
    pub player: Arc<Player>,
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub item_in_hand: ItemStack,
}

impl BlockDamageAbortEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        block_pos: BlockPos,
        world: Arc<World>,
        item_in_hand: ItemStack,
    ) -> Self {
        Self {
            player,
            block_pos,
            world,
            item_in_hand,
        }
    }
}
