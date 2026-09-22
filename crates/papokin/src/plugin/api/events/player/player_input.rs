use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 收到玩家输入时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInputEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 输入描述符。
    pub input: String,
}

impl PlayerInputEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, input: String) -> Self {
        Self {
            player,
            input,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerInputEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
