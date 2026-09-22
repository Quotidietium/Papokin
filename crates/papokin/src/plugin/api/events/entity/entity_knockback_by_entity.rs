use papokin_macros::{Event, cancellable};

/// 实体被另一实体击退时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityKnockbackByEntityEvent {
    /// 承受击退的实体 ID。
    pub entity_id: i32,
    /// 造成击退的实体 ID。
    pub hit_by_id: i32,
    /// 击退力度。
    pub force: f64,
    /// X 方向速度变化。
    pub x: f64,
    /// Z 方向速度变化。
    pub z: f64,
}

impl EntityKnockbackByEntityEvent {
    #[must_use]
    pub const fn new(entity_id: i32, hit_by_id: i32, force: f64, x: f64, z: f64) -> Self {
        Self {
            entity_id,
            hit_by_id,
            force,
            x,
            z,
            cancelled: false,
        }
    }
}
