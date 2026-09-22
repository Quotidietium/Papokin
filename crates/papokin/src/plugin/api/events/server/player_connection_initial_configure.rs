use papokin_macros::Event;
use uuid::Uuid;

/// 玩家连接初次被配置时发生的事件
/// (配置阶段完成时，玩家进入世界之前)。
#[derive(Event, Clone)]
pub struct PlayerConnectionInitialConfigureEvent {
    /// 正在连接的玩家名称。
    pub player_name: String,

    /// 正在连接的玩家的 UUID。
    pub player_uuid: Uuid,

    /// 玩家是否首次加入。
    pub first_join: bool,
}

impl PlayerConnectionInitialConfigureEvent {
    #[must_use]
    pub const fn new(player_name: String, player_uuid: Uuid, first_join: bool) -> Self {
        Self {
            player_name,
            player_uuid,
            first_join,
        }
    }
}
