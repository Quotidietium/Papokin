use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 物品栏交互期间触发的基础事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryInteractEvent {
    /// 与物品栏交互的玩家。
    pub player: Arc<Player>,
}

impl InventoryInteractEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}
