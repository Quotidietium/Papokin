use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::sync::Arc;

/// 实体死亡时发生的事件。
#[derive(Event, Clone)]
pub struct EntityDeathEvent {
    /// 死亡的实体 ID。
    pub entity_id: i32,

    /// 实体掉落的经验数量。
    pub dropped_exp: i32,
}

impl EntityDeathEvent {
    #[must_use]
    pub const fn new(entity_id: i32, dropped_exp: i32) -> Self {
        Self {
            entity_id,
            dropped_exp,
        }
    }
}

/// 玩家死亡时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerDeathEvent {
    /// 死亡的玩家。
    pub player: Arc<Player>,

    /// 要广播的死亡消息。
    pub death_message: TextComponent,

    /// 玩家掉落的经验数量。
    pub dropped_exp: i32,

    /// 玩家死亡时是否保留物品栏。
    pub keep_inventory: bool,
}

impl PlayerDeathEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, death_message: TextComponent, dropped_exp: i32) -> Self {
        Self {
            player,
            death_message,
            dropped_exp,
            keep_inventory: false,
            cancelled: false,
        }
    }
}
