use papokin_macros::{Event, cancellable};

/// 实体的剩余氧气量变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityAirChangeEvent {
    /// 实体的 ID。
    pub entity_id: i32,

    /// 新的氧气量。
    pub amount: i32,
}

impl EntityAirChangeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, amount: i32) -> Self {
        Self {
            entity_id,
            amount,
            cancelled: false,
        }
    }
}
