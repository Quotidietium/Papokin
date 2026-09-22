use papokin_macros::Event;

/// 命令注册到服务器时发生的事件。
#[derive(Event, Clone)]
pub struct CommandRegisteredEvent {
    /// 已注册命令的标签。
    pub command_label: String,

    /// 注册该命令的插件名称。
    pub plugin: String,
}

impl CommandRegisteredEvent {
    #[must_use]
    pub const fn new(command_label: String, plugin: String) -> Self {
        Self {
            command_label,
            plugin,
        }
    }
}
