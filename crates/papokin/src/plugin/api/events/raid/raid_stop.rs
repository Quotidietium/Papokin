use papokin_macros::{Event, cancellable};

/// 袭击停止时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct RaidStopEvent {
    /// 袭击停止的原因。
    pub reason: String,
}

impl RaidStopEvent {
    #[must_use]
    pub const fn new(reason: String) -> Self {
        Self {
            reason,
            cancelled: false,
        }
    }
}
