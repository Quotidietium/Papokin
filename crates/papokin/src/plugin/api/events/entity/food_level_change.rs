use papokin_macros::{Event, cancellable};

/// 实体的饥饿值变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct FoodLevelChangeEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 新的饥饿值。
    pub food_level: u8,
}

impl FoodLevelChangeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, food_level: u8) -> Self {
        Self {
            entity_id,
            food_level,
            cancelled: false,
        }
    }
}
