use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 实体被玩家驯服时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTameEvent {
    /// 已驯服实体的 ID。
    pub entity_id: i32,

    /// 驯服实体的玩家。
    pub owner: Arc<Player>,
}

impl EntityTameEvent {
    #[must_use]
    pub const fn new(entity_id: i32, owner: Arc<Player>) -> Self {
        Self {
            entity_id,
            owner,
            cancelled: false,
        }
    }
}
