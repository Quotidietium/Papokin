use papokin_macros::{Event, cancellable};

/// 插件被启用时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PluginEnableEvent {
    /// 被启用的插件的名称。
    pub plugin_name: String,
}

impl PluginEnableEvent {
    #[must_use]
    pub const fn new(plugin_name: String) -> Self {
        Self {
            plugin_name,
            cancelled: false,
        }
    }
}
