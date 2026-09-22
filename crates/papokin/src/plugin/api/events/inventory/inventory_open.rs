use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家打开物品栏时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryOpenEvent {
    /// 打开物品栏的玩家。
    pub player: Arc<Player>,
}

impl InventoryOpenEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}
