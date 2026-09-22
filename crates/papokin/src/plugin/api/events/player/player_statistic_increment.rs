use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家统计值增加时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStatisticIncrementEvent {
    /// 统计数据发生变化的玩家。
    pub player: Arc<Player>,

    /// 统计项标识符名称。
    pub statistic_id: String,

    /// 递增的量。
    pub amount: i32,
}

impl PlayerEvent for PlayerStatisticIncrementEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
