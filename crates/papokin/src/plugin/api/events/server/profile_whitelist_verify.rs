use papokin_macros::Event;
use papokin_util::text::TextComponent;
use uuid::Uuid;

/// 白名单校验的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhitelistVerifyResult {
    /// 允许该玩家加入。
    Allowed,
    /// 拒绝该玩家；客户端将被踢出并显示踢出消息。
    Denied,
}

/// 正在连接的玩家被验证时发生的事件，验证所对照的
/// 白名单。
///
/// 事件触发时会携带原版判定结果；处理器可以修改
/// [`Self::result`] 来覆盖它，并用 [`Self::kick_message`] 自定义
/// 玩家不在白名单中时显示的断开连接消息。
#[derive(Event, Clone)]
pub struct ProfileWhitelistVerifyEvent {
    /// 正在连接的玩家的 UUID。
    pub player_uuid: Uuid,

    /// 正在连接的玩家名称。
    pub player_name: String,

    /// 玩家不在白名单时发送的踢出消息。
    pub kick_message: TextComponent,

    /// 验证结果。
    pub result: WhitelistVerifyResult,
}

impl ProfileWhitelistVerifyEvent {
    #[must_use]
    pub const fn new(
        player_uuid: Uuid,
        player_name: String,
        kick_message: TextComponent,
        result: WhitelistVerifyResult,
    ) -> Self {
        Self {
            player_uuid,
            player_name,
            kick_message,
            result,
        }
    }
}
