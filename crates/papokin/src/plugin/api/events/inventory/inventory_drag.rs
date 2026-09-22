use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家跨物品栏槽位拖动物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryDragEvent {
    /// 拖动物品的玩家。
    pub player: Arc<Player>,
}

impl InventoryDragEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}
