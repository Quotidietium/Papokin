use papokin_macros::Event;
use papokin_util::Hand;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家更改主手时发生的事件。
#[derive(Event, Clone)]
pub struct PlayerChangedMainHandEvent {
    /// 主手发生变化的玩家。
    pub player: Arc<Player>,

    /// 玩家的新主手。
    pub main_hand: Hand,
}

impl PlayerChangedMainHandEvent {
    /// 创建 `PlayerChangedMainHandEvent` 的新实例。
    pub const fn new(player: Arc<Player>, main_hand: Hand) -> Self {
        Self { player, main_hand }
    }
}

impl PlayerEvent for PlayerChangedMainHandEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
