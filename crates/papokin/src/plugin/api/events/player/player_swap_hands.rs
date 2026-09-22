use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家在主手与副手之间交换物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSwapHandItemsEvent {
    /// 交换主副手物品的玩家。
    pub player: Arc<Player>,
}

impl PlayerSwapHandItemsEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}
