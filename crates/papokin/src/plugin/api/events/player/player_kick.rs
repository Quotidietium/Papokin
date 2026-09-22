use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家被踢出服务器时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerKickEvent {
    /// 被踢出的玩家。
    pub player: Arc<Player>,

    /// 踢出的原因。
    pub reason: String,
}

impl PlayerKickEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, reason: String) -> Self {
        Self {
            player,
            reason,
            cancelled: false,
        }
    }
}
