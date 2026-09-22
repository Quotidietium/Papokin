use papokin_macros::{Event, cancellable};

/// 实体被解开拴绳时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityUnleashEvent {
    /// 正在被解除拴绳的实体 ID。
    pub entity_id: i32,
    /// 解除拴绳的原因。
    pub reason: String,
}

impl EntityUnleashEvent {
    #[must_use]
    pub const fn new(entity_id: i32, reason: String) -> Self {
        Self {
            entity_id,
            reason,
            cancelled: false,
        }
    }
}
