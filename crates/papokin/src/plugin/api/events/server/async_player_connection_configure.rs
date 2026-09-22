use papokin_macros::Event;
use uuid::Uuid;

/// 玩家连接正在被配置时异步发生的事件
/// 已配置（在配置阶段开始时）。
#[derive(Event, Clone)]
pub struct AsyncPlayerConnectionConfigureEvent {
    /// 正在连接的玩家名称。
    pub player_name: String,

    /// 正在连接的玩家的 UUID。
    pub player_uuid: Uuid,

    /// 玩家是否首次加入。
    pub first_join: bool,
}

impl AsyncPlayerConnectionConfigureEvent {
    #[must_use]
    pub const fn new(player_name: String, player_uuid: Uuid, first_join: bool) -> Self {
        Self {
            player_name,
            player_uuid,
            first_join,
        }
    }
}
