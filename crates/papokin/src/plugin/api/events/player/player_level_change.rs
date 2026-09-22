use crate::entity::player::Player;
use papokin_macros::Event;
use std::sync::Arc;

/// 玩家的经验等级变化时发生的事件。
#[derive(Event, Clone)]
pub struct PlayerLevelChangeEvent {
    /// 等级发生变化的玩家。
    pub player: Arc<Player>,

    /// 原等级。
    pub old_level: i32,

    /// 新的等级。
    pub new_level: i32,
}

impl PlayerLevelChangeEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, old_level: i32, new_level: i32) -> Self {
        Self {
            player,
            old_level,
            new_level,
        }
    }
}
