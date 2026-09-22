use papokin_macros::Event;

/// [`ServerLoadEvent`] 触发的原因。
#[derive(Clone, Copy, Debug)]
pub enum LoadType {
    /// 服务器完成了正常的启动流程。
    Startup,
    /// 服务器完成了完整的重载流程。
    Reload,
}

/// 服务器完成加载后触发的事件。
#[derive(Event, Clone)]
pub struct ServerLoadEvent {
    /// 触发服务器加载事件的原因。
    pub load_type: LoadType,
}

impl ServerLoadEvent {
    /// 创建新的 `ServerLoadEvent`。
    #[must_use]
    pub const fn new(load_type: LoadType) -> Self {
        Self { load_type }
    }
}
