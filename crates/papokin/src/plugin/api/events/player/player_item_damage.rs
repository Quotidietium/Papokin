use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家手持或穿戴的物品受到耐久损耗时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemDamageEvent {
    /// 物品正在受损的玩家。
    pub player: Arc<Player>,

    /// 受到伤害的物品的注册表名称。
    pub item_name: String,

    /// 耐久损耗量。
    pub damage: i32,
}

impl PlayerItemDamageEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, item_name: String, damage: i32) -> Self {
        Self {
            player,
            item_name,
            damage,
            cancelled: false,
        }
    }
}
