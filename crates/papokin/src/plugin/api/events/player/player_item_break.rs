use crate::entity::player::Player;
use papokin_macros::Event;
use std::sync::Arc;

/// 玩家损坏物品时触发的事件。
#[derive(Event, Clone)]
pub struct PlayerItemBreakEvent {
    /// 物品损坏的玩家。
    pub player: Arc<Player>,

    /// 被破坏物品的注册表键。
    pub item_name: String,
}

impl PlayerItemBreakEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, item_name: String) -> Self {
        Self { player, item_name }
    }
}
