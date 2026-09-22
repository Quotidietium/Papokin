use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家获得经验值时发生的事件。
#[derive(Event, Clone)]
pub struct PlayerExpChangeEvent {
    /// 获得经验的玩家。
    pub player: Arc<Player>,

    /// 要增加的经验数量。
    pub amount: i32,
}

impl PlayerExpChangeEvent {
    /// 创建 `PlayerExpChangeEvent` 的新实例。
    pub const fn new(player: Arc<Player>, amount: i32) -> Self {
        Self { player, amount }
    }
}

impl PlayerEvent for PlayerExpChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
