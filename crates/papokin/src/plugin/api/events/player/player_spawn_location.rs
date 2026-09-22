use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

/// 确定玩家出生位置时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSpawnLocationEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 生成位置。
    pub spawn_pos: Vector3<f64>,
}

impl PlayerSpawnLocationEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, spawn_pos: Vector3<f64>) -> Self {
        Self {
            player,
            spawn_pos,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerSpawnLocationEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
