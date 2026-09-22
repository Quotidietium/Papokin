use papokin_macros::{Event, cancellable};

/// 实体经历饥饿消耗时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityExhaustionEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 增加的消耗值。
    pub exhaustion: f32,
}

impl EntityExhaustionEvent {
    #[must_use]
    pub const fn new(entity_id: i32, exhaustion: f32) -> Self {
        Self {
            entity_id,
            exhaustion,
            cancelled: false,
        }
    }
}
