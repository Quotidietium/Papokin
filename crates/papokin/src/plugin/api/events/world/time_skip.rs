use papokin_macros::{Event, cancellable};

/// 世界时间被跳过时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct TimeSkipEvent {
    /// 跳过的刻数。
    pub skip_amount: i64,
}

impl TimeSkipEvent {
    #[must_use]
    pub const fn new(skip_amount: i64) -> Self {
        Self {
            skip_amount,
            cancelled: false,
        }
    }
}
