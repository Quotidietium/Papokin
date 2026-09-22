use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家消耗物品（食物、药水等）时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemConsumeEvent {
    /// 消耗该物品的玩家。
    pub player: Arc<Player>,

    /// 被消耗物品的注册表名称。
    pub item_name: String,
}

impl PlayerItemConsumeEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, item_name: String) -> Self {
        Self {
            player,
            item_name,
            cancelled: false,
        }
    }
}
