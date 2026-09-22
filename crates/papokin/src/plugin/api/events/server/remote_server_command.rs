use papokin_macros::{Event, cancellable};

/// 通过远程控制台（RCON）执行命令时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct RemoteServerCommandEvent {
    /// 已执行的命令行。
    pub command: String,
}

impl RemoteServerCommandEvent {
    #[must_use]
    pub const fn new(command: String) -> Self {
        Self {
            command,
            cancelled: false,
        }
    }
}
