use std::sync::Arc;

use papokin_macros::{Event, cancellable};

use crate::entity::player::Player;

/// 末影人攻击玩家时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EndermanAttackPlayerEvent {
    /// 发起攻击的末影人 ID。
    pub entity_id: i32,

    /// 被攻击的玩家。
    pub player: Arc<Player>,
}

impl EndermanAttackPlayerEvent {
    #[must_use]
    pub const fn new(entity_id: i32, player: Arc<Player>) -> Self {
        Self {
            entity_id,
            player,
            cancelled: false,
        }
    }
}
