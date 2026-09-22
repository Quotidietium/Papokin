use std::sync::Arc;

use papokin_macros::{Event, cancellable};

use crate::entity::player::Player;

/// 实体（海龟）为蛋受精时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityFertilizeEggEvent {
    /// 产蛋实体的 ID。
    pub entity_id: i32,

    /// 繁殖该实体的玩家（如果有的话）。
    pub breeder: Option<Arc<Player>>,
}

impl EntityFertilizeEggEvent {
    #[must_use]
    pub const fn new(entity_id: i32, breeder: Option<Arc<Player>>) -> Self {
        Self {
            entity_id,
            breeder,
            cancelled: false,
        }
    }
}
