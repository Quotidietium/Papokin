use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家通过末地折跃门传送时发生的事件。
///
/// 取消即否决传送。宿主端尚未接线（最终
/// 折跃门方块已被其他代理拥有）；集成说明参见
/// 事件接线报告。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerTeleportEndGatewayEvent {
    /// 正在传送的玩家。
    pub player: Arc<Player>,

    /// 末地折跃门方块的位置。
    pub gateway: BlockPos,

    /// 玩家传送的起始位置。
    pub from_position: Vector3<f64>,

    /// 玩家传送的目标位置。
    pub to_position: Vector3<f64>,
}

impl PlayerTeleportEndGatewayEvent {
    /// 创建 `PlayerTeleportEndGatewayEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        gateway: BlockPos,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            player,
            gateway,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerTeleportEndGatewayEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
