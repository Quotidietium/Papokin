use papokin_macros::Event;
use uuid::Uuid;

/// 服务器满员加入检查的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ServerFullCheckResult {
    /// 即使服务器已满也允许该玩家加入。
    Allowed,

    /// 拒绝该玩家并踢出（默认）。
    #[default]
    Denied,
}

/// 玩家尝试加入已满员的服务器时发生的事件。
///
/// 只有在服务器真正满员时才会触发此事件。设置
/// `result` 设为 [`ServerFullCheckResult::Allowed`] 即可让玩家加入
/// 无论如何；默认值为 [`ServerFullCheckResult::Denied`]。
///
/// 该连接尚无玩家对象，因此此事件不会
/// 实现 `PlayerEvent`。这是 Java 协议的登录事件。
#[derive(Event, Clone)]
pub struct PlayerServerFullCheckEvent {
    /// 正在加入的玩家名称。
    pub player_name: String,

    /// 正在加入的玩家的 UUID。
    pub player_uuid: Uuid,

    /// 即使服务器已满，该玩家是否仍被允许加入。
    pub result: ServerFullCheckResult,
}

impl PlayerServerFullCheckEvent {
    /// 创建 `PlayerServerFullCheckEvent` 的新实例。
    pub fn new(player_name: impl Into<String>, player_uuid: Uuid) -> Self {
        Self {
            player_name: player_name.into(),
            player_uuid,
            result: ServerFullCheckResult::Denied,
        }
    }
}
