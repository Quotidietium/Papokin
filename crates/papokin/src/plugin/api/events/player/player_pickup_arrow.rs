use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家拾起箭时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickupArrowEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 箭实体 ID。
    pub arrow_id: i32,
}

impl PlayerPickupArrowEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, arrow_id: i32) -> Self {
        Self {
            player,
            arrow_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickupArrowEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
