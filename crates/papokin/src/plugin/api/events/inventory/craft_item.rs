use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家合成物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CraftItemEvent {
    /// 合成该物品的玩家。
    pub player: Arc<Player>,

    /// 配方标识符。
    pub recipe_id: String,
}

impl CraftItemEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, recipe_id: String) -> Self {
        Self {
            player,
            recipe_id,
            cancelled: false,
        }
    }
}
