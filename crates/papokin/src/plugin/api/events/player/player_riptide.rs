use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家激活三叉戟的激流附魔时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerRiptideEvent {
    /// 激活激流附魔的玩家。
    pub player: Arc<Player>,

    /// 激流物品名称。
    pub item_name: String,
}

impl PlayerEvent for PlayerRiptideEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
