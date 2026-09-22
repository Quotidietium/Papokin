use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家跳跃时发生的事件。
///
/// 客户端不会发送跳跃目的地，因此 `to_position` 会镜像
/// `from_position`（与 Paper 一致）。取消事件会跳过服务器端效果
/// 跳跃相关部分（跳跃统计与消耗）；物理运动本身
/// 由客户端权威决定，无法在此处回滚。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerJumpEvent {
    /// 跳跃的玩家。
    pub player: Arc<Player>,

    /// 玩家跳跃的起始位置。
    pub from_position: Vector3<f64>,

    /// 玩家跳跃的目标位置。
    pub to_position: Vector3<f64>,
}

impl PlayerJumpEvent {
    /// 创建 `PlayerJumpEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            player,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerJumpEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
