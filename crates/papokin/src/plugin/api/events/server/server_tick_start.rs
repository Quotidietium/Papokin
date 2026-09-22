use papokin_macros::Event;

/// 每次服务器刻开始时触发的事件。
#[derive(Event, Clone)]
pub struct ServerTickStartEvent {
    /// 即将运行的刻的编号（从 0 开始）。
    pub tick: i32,
}

impl ServerTickStartEvent {
    /// 创建新的 `ServerTickStartEvent`。
    #[must_use]
    pub const fn new(tick: i32) -> Self {
        Self { tick }
    }
}
