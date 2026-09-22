use papokin_macros::Event;

/// 实体跳跃时发生的事件。
#[derive(Event, Clone)]
pub struct EntityJumpEvent {
    /// 跳跃的实体 ID。
    pub entity_id: i32,
}

impl EntityJumpEvent {
    #[must_use]
    pub const fn new(entity_id: i32) -> Self {
        Self { entity_id }
    }
}
