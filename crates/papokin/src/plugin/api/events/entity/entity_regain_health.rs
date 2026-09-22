use papokin_macros::{Event, cancellable};

/// 实体恢复生命值时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityRegainHealthEvent {
    /// 恢复生命的实体 ID。
    pub entity_id: i32,

    /// 恢复的生命值数量。
    pub amount: f32,
}

impl EntityRegainHealthEvent {
    #[must_use]
    pub const fn new(entity_id: i32, amount: f32) -> Self {
        Self {
            entity_id,
            amount,
            cancelled: false,
        }
    }
}
