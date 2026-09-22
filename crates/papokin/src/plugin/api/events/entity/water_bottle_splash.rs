use papokin_macros::{Event, cancellable};

/// 水瓶溅到实体上时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WaterBottleSplashEvent {
    /// 被投掷的药水实体的 ID。
    pub entity_id: i32,

    /// 受喷溅影响的所有实体的 ID。
    pub affected_entities: Vec<i32>,
}

impl WaterBottleSplashEvent {
    #[must_use]
    pub const fn new(entity_id: i32, affected_entities: Vec<i32>) -> Self {
        Self {
            entity_id,
            affected_entities,
            cancelled: false,
        }
    }
}
