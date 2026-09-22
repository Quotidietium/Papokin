use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家攻击冷却因攻击
/// 一个实体。
///
/// 取消会保留之前的冷却进度，因此该攻击会被视为
/// 就好像冷却尚未重置一样。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerAttackEntityCooldownResetEvent {
    /// 发起攻击的玩家。
    pub player: Arc<Player>,

    /// 正在被攻击的实体的实体 ID。
    pub target_id: i32,
}

impl PlayerAttackEntityCooldownResetEvent {
    /// 创建 `PlayerAttackEntityCooldownResetEvent` 的新实例。
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerAttackEntityCooldownResetEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
