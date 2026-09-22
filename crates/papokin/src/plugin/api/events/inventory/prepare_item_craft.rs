use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 在合成网格中准备配方时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PrepareItemCraftEvent {
    /// 合成该物品的玩家。
    pub player: Arc<Player>,

    /// 准备好的配方 ID。
    pub recipe_id: String,
}

impl PrepareItemCraftEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, recipe_id: String) -> Self {
        Self {
            player,
            recipe_id,
            cancelled: false,
        }
    }
}
