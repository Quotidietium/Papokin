use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 实体受到击退时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityKnockbackEvent {
    /// 受到击退的实体 ID。
    pub entity_id: i32,

    /// 造成击退的实体 ID（如有）。
    pub hit_by_id: Option<i32>,

    /// 击退向量。
    pub knockback: Vector3<f64>,
}
