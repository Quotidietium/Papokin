use papokin_macros::{Event, cancellable};

/// 服务被注销时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ServiceUnregisterEvent {
    /// 服务的名称。
    pub service_name: String,
}

impl ServiceUnregisterEvent {
    #[must_use]
    pub const fn new(service_name: String) -> Self {
        Self {
            service_name,
            cancelled: false,
        }
    }
}
