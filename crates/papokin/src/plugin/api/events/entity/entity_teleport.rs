use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 实体传送时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTeleportEvent {
    /// 实体 ID。
    pub entity_id: i32,

    /// 起始位置。
    pub from_position: Vector3<f64>,

    /// 目标位置。
    pub to_position: Vector3<f64>,
}

impl EntityTeleportEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            entity_id,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}
