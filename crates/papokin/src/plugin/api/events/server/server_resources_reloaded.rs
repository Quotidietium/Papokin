use papokin_macros::Event;

/// 服务器资源（数据包）重新加载时发生的事件。
#[derive(Event, Clone)]
pub struct ServerResourcesReloadedEvent {
    /// 重载的原因（例如命令或插件）。
    pub cause: String,
}

impl ServerResourcesReloadedEvent {
    #[must_use]
    pub const fn new(cause: String) -> Self {
        Self { cause }
    }
}
