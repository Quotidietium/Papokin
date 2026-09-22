use papokin_macros::{Event, cancellable};
use papokin_util::math::{position::BlockPos, vector3::Vector3};

/// 实体通过末地折跃门传送时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTeleportEndGatewayEvent {
    /// 正在传送的实体的 ID。
    pub entity_id: i32,

    /// 末地折跃门方块的位置。
    pub gateway: BlockPos,

    /// 实体传送的起始位置。
    pub from_position: Vector3<f64>,

    /// 实体传送的目标位置。
    pub to_position: Vector3<f64>,
}

impl EntityTeleportEndGatewayEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        gateway: BlockPos,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            entity_id,
            gateway,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}
