use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家采集方块（如甜浆果丛、蜂巢）时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerHarvestBlockEvent {
    /// 收割方块的玩家。
    pub player: Arc<Player>,

    /// 被收割方块的位置。
    pub block_pos: BlockPos,

    /// 收获到的物品。
    pub harvested_items: Vec<ItemStack>,
}

impl PlayerEvent for PlayerHarvestBlockEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
