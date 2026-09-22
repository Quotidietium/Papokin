use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::super::player::PlayerEvent;

/// 玩家的对话框被清除时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct DialogClearEvent {
    /// 对话框正被清除的玩家。
    pub player: Arc<Player>,
}

impl DialogClearEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}

impl PlayerEvent for DialogClearEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
