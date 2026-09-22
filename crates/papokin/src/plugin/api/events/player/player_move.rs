use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家移动时触发的事件。
///
/// 如果事件被取消，玩家将无法移动。
///
/// 此事件包含关于玩家、玩家移动的起始位置和移动到的位置的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerMoveEvent {
    /// 发生移动的玩家。
    pub player: Arc<Player>,

    /// 玩家移动前的位置。
    pub from: Vector3<f64>,

    /// 玩家移动后的位置。
    pub to: Vector3<f64>,
}

impl PlayerMoveEvent {
    /// 创建 `PlayerMoveEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：发生移动的玩家的引用。
    /// - `from`：玩家移动前的位置。
    /// - `to`：玩家移动到的位置。
    ///
    /// # Returns
    /// 一个新的 `PlayerMoveEvent` 实例。
    pub const fn new(player: Arc<Player>, from: Vector3<f64>, to: Vector3<f64>) -> Self {
        Self {
            player,
            from,
            to,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerMoveEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
