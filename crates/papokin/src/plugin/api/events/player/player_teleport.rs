use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家传送时发生的事件。
///
/// 如果事件被取消，传送将不会发生。
///
/// 此事件包含关于玩家、玩家传送的起始位置和传送到的位置的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerTeleportEvent {
    /// 进行传送的玩家。
    pub player: Arc<Player>,

    /// 玩家传送前的位置。
    pub from: Vector3<f64>,

    /// 玩家传送后的位置。
    pub to: Vector3<f64>,
}

impl PlayerTeleportEvent {
    /// 创建 `PlayerTeleportEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：发生传送的玩家的引用。
    /// - `from`：玩家传送前的位置。
    /// - `to`：玩家传送到的位置。
    ///
    /// # Returns
    /// 一个新的 `PlayerTeleportEvent` 实例。
    pub const fn new(player: Arc<Player>, from: Vector3<f64>, to: Vector3<f64>) -> Self {
        Self {
            player,
            from,
            to,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerTeleportEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
