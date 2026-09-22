use papokin_macros::{Event, cancellable};

/// 插件被禁用时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PluginDisableEvent {
    /// 被禁用的插件的名称。
    pub plugin_name: String,
}

impl PluginDisableEvent {
    #[must_use]
    pub const fn new(plugin_name: String) -> Self {
        Self {
            plugin_name,
            cancelled: false,
        }
    }
}
