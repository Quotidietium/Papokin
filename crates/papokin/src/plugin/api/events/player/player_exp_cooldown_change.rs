use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家经验冷却变化时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerExpCooldownChangeEvent {
    /// 相关的玩家。
    pub player: Arc<Player>,

    /// 新的冷却刻数。
    pub new_cooldown: i32,
}

impl PlayerEvent for PlayerExpCooldownChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
