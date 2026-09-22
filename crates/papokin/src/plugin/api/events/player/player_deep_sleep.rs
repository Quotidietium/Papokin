use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家进入深度睡眠时发生的事件（睡眠 100 刻，
/// 原版开始将玩家计入睡眠
/// 百分比，且幻翼生成被抑制）。
///
/// 纯通知；每个睡眠周期触发一次。
#[derive(Event, Clone)]
pub struct PlayerDeepSleepEvent {
    /// 进入深度睡眠的玩家。
    pub player: Arc<Player>,
}

impl PlayerDeepSleepEvent {
    /// 创建 `PlayerDeepSleepEvent` 的新实例。
    pub const fn new(player: Arc<Player>) -> Self {
        Self { player }
    }
}

impl PlayerEvent for PlayerDeepSleepEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
