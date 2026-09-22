use papokin_macros::{Event, cancellable};

/// 实体从世界移除时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityRemoveEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 移除原因。
    pub cause: String,
}

impl EntityRemoveEvent {
    #[must_use]
    pub const fn new(entity_id: i32, cause: String) -> Self {
        Self {
            entity_id,
            cause,
            cancelled: false,
        }
    }
}
