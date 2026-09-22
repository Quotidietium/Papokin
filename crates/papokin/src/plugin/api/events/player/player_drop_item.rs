use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家丢弃物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerDropItemEvent {
    /// 丢弃该物品的玩家。
    pub player: Arc<Player>,

    /// 掉落物品的注册表名称。
    pub item_name: String,

    /// 掉落物品的堆叠数量。
    pub count: u8,
}

impl PlayerDropItemEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, item_name: String, count: u8) -> Self {
        Self {
            player,
            item_name,
            count,
            cancelled: false,
        }
    }
}
