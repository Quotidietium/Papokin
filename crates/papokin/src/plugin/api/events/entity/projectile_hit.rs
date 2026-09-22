use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 弹射物击中实体或方块时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ProjectileHitEvent {
    /// 投射物实体的 ID。
    pub entity_id: i32,
    /// 命中位置。
    pub hit_position: Vector3<f64>,
    /// 被击中实体的 ID（如适用）。
    pub hit_entity_id: Option<i32>,
}

impl ProjectileHitEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        hit_position: Vector3<f64>,
        hit_entity_id: Option<i32>,
    ) -> Self {
        Self {
            entity_id,
            hit_position,
            hit_entity_id,
            cancelled: false,
        }
    }
}
