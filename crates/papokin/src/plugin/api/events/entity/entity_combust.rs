use papokin_macros::{Event, cancellable};

/// 实体着火时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityCombustEvent {
    /// 着火的实体 ID。
    pub entity_id: i32,

    /// 实体燃烧的时长（以秒为单位）。
    pub duration_secs: f32,
}

impl EntityCombustEvent {
    #[must_use]
    pub const fn new(entity_id: i32, duration_secs: f32) -> Self {
        Self {
            entity_id,
            duration_secs,
            cancelled: false,
        }
    }
}
