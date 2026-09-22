use papokin_macros::{Event, cancellable};

/// 实体的姿态变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPoseChangeEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 新的姿态名称。
    pub pose: String,
}

impl EntityPoseChangeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, pose: String) -> Self {
        Self {
            entity_id,
            pose,
            cancelled: false,
        }
    }
}
