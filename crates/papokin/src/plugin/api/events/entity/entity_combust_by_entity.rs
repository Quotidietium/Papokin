use papokin_macros::{Event, cancellable};

/// 实体被另一实体点燃时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityCombustByEntityEvent {
    /// 正在被燃烧的实体 ID。
    pub entity_id: i32,
    /// 引燃实体 ID。
    pub combuster_id: i32,
    /// 燃烧持续时间（秒）。
    pub duration: f32,
}

impl EntityCombustByEntityEvent {
    #[must_use]
    pub const fn new(entity_id: i32, combuster_id: i32, duration: f32) -> Self {
        Self {
            entity_id,
            combuster_id,
            duration,
            cancelled: false,
        }
    }
}
