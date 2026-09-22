use papokin_macros::Event;
use uuid::Uuid;

/// 白名单状态更新的种类。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhitelistStateUpdateStatus {
    /// 玩家已被加入白名单。
    Added,
    /// 玩家已被移出白名单。
    Removed,
}

/// 玩家的白名单状态更新时发生的事件。
#[derive(Event, Clone)]
pub struct WhitelistStateUpdateEvent {
    /// 目标玩家的名称。
    pub player_name: String,

    /// 目标玩家的 UUID。
    pub player_uuid: Uuid,

    /// 目标的新白名单状态。
    pub status: WhitelistStateUpdateStatus,
}

impl WhitelistStateUpdateEvent {
    #[must_use]
    pub const fn new(
        player_name: String,
        player_uuid: Uuid,
        status: WhitelistStateUpdateStatus,
    ) -> Self {
        Self {
            player_name,
            player_uuid,
            status,
        }
    }
}
