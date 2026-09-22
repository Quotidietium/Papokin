use papokin_macros::{Event, cancellable};

/// 末影人逃离战斗时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EndermanEscapeEvent {
    /// 逃走的末影人 ID。
    pub entity_id: i32,

    /// 逃脱原因（例如 `teleport` 或 `unknown`）。
    pub reason: String,
}

impl EndermanEscapeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, reason: String) -> Self {
        Self {
            entity_id,
            reason,
            cancelled: false,
        }
    }
}
