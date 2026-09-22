use papokin_macros::Event;

/// 每个服务器刻结束时触发的事件。
#[derive(Event, Clone)]
pub struct ServerTickEndEvent {
    /// 刚刚结束的刻的编号（从 0 开始）。
    pub tick: i32,

    /// 刚结束的这一刻的时长（以纳秒计）。
    pub duration_nanos: i64,
}

impl ServerTickEndEvent {
    /// 创建新的 `ServerTickEndEvent`。
    #[must_use]
    pub const fn new(tick: i32, duration_nanos: i64) -> Self {
        Self {
            tick,
            duration_nanos,
        }
    }
}
