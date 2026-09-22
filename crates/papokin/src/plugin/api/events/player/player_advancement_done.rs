use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家完成进度时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerAdvancementDoneEvent {
    /// 完成进度的玩家。
    pub player: Arc<Player>,

    /// 进度标识符。
    pub advancement_id: String,
}

impl PlayerAdvancementDoneEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, advancement_id: String) -> Self {
        Self {
            player,
            advancement_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerAdvancementDoneEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
