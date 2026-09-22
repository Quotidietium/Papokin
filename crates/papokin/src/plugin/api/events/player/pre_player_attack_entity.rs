use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家攻击实体前发生的事件，允许
/// 在计算任何伤害之前被取消。
#[cancellable]
#[derive(Event, Clone)]
pub struct PrePlayerAttackEntityEvent {
    /// 发起攻击的玩家。
    pub player: Arc<Player>,

    /// 即将被攻击的实体的实体 ID。
    pub target_id: i32,
}

impl PrePlayerAttackEntityEvent {
    /// 创建 `PrePlayerAttackEntityEvent` 的新实例。
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PrePlayerAttackEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
