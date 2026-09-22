use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;

/// 有内容尝试向服务器广播消息时发生的事件。
///
/// 此事件包含关于正在广播的消息的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct ServerBroadcastEvent {
    /// 正在广播的消息。
    pub message: TextComponent,
    /// 以 `TextComponent` 表示的发送者名称。
    pub sender: TextComponent,
}

impl ServerBroadcastEvent {
    /// 创建 `ServerBroadcastEvent` 的新实例。
    ///
    /// # Arguments
    /// - `message`：正在广播的消息。
    /// - `sender`：以 `TextComponent` 表示的发送者名称。
    ///
    /// # Returns
    /// 一个新的 `ServerBroadcastEvent` 实例。
    #[must_use]
    pub const fn new(message: TextComponent, sender: TextComponent) -> Self {
        Self {
            message,
            sender,
            cancelled: false,
        }
    }
}
