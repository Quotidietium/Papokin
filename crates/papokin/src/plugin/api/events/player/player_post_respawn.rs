use papokin_macros::Event;
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家重生后发生的事件。
///
/// 重生完成后触发一次的纯通知；仅用于观察
/// 与前文对应的
/// [`super::player_respawn::PlayerRespawnEvent`]。
#[derive(Event, Clone)]
pub struct PlayerPostRespawnEvent {
    /// 重生后的玩家。
    pub player: Arc<Player>,

    /// 玩家重生所在的位置。
    pub respawn_location: Vector3<f64>,

    /// 重生位置是否为玩家的床/重生锚生成点。
    pub is_bed_spawn: bool,
}

impl PlayerPostRespawnEvent {
    /// 创建 `PlayerPostRespawnEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        respawn_location: Vector3<f64>,
        is_bed_spawn: bool,
    ) -> Self {
        Self {
            player,
            respawn_location,
            is_bed_spawn,
        }
    }
}

impl PlayerEvent for PlayerPostRespawnEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
