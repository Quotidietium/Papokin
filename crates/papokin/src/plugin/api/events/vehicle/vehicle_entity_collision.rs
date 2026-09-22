use papokin_macros::{Event, cancellable};

/// 载具与另一个实体碰撞时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleEntityCollisionEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 碰撞实体的 ID。
    pub collided_entity_id: i32,
}

impl VehicleEntityCollisionEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, collided_entity_id: i32) -> Self {
        Self {
            vehicle_id,
            collided_entity_id,
            cancelled: false,
        }
    }
}
