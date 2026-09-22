use papokin_macros::Event;
use papokin_util::text::TextComponent;

/// 登录验证的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionValidationResult {
    /// 允许登录继续进行。
    Allowed,
    /// 登录被拒绝；客户端会被踢出并收到踢出消息。
    Denied,
}

/// 传入的登录被验证、可据此接受或拒绝时发生的事件
/// 提前拒绝，或覆盖已作出的拒绝。
///
/// 事件触发时会携带原版验证的判定结果；处理器可以修改
/// [`Self::result`] 来覆盖它，并用 [`Self::kick_message`] 自定义
/// 登录被拒绝时显示的断开连接消息。
#[derive(Event, Clone)]
pub struct PlayerConnectionValidateLoginEvent {
    /// 正在连接的客户端的 IP 地址。
    pub ip_address: String,

    /// 登录被拒绝时发送的踢出消息。
    pub kick_message: TextComponent,

    /// 校验结果。
    pub result: ConnectionValidationResult,
}

impl PlayerConnectionValidateLoginEvent {
    #[must_use]
    pub const fn new(
        ip_address: String,
        kick_message: TextComponent,
        result: ConnectionValidationResult,
    ) -> Self {
        Self {
            ip_address,
            kick_message,
            result,
        }
    }
}
