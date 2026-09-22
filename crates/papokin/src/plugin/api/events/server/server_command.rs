use papokin_macros::{Event, cancellable};

/// 命令被发送到服务器控制台时发生的事件。
///
/// 此事件包含关于正在执行的命令的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct ServerCommandEvent {
    /// 正在执行的命令。
    pub command: String,
}

impl ServerCommandEvent {
    /// 创建 `ServerCommandEvent` 的新实例。
    ///
    /// # Arguments
    /// * `command` - 正在执行的命令。
    ///
    /// # Returns
    /// 一个新的 `ServerCommandEvent` 实例。
    #[must_use]
    pub const fn new(command: String) -> Self {
        Self {
            command,
            cancelled: false,
        }
    }
}
