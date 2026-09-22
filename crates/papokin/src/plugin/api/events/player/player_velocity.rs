use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家速度变化时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerVelocityEvent {
    /// 相关的玩家。
    pub player: Arc<Player>,

    /// 新的速度向量。
    pub velocity: Vector3<f64>,
}

impl PlayerEvent for PlayerVelocityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
