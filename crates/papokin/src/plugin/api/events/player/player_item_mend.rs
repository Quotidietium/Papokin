use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家物品被经验修补时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemMendEvent {
    /// 拥有该物品的玩家。
    pub player: Arc<Player>,

    /// 被修复物品的名称。
    pub item_name: String,

    /// 已修复的耐久点数。
    pub repair_amount: i32,

    /// 消耗的经验点数。
    pub exp_consumed: i32,
}

impl PlayerEvent for PlayerItemMendEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
