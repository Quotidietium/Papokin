use papokin_macros::{Event, cancellable};

/// 载具碰撞时触发的基础事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleCollisionEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
}

impl VehicleCollisionEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32) -> Self {
        Self {
            vehicle_id,
            cancelled: false,
        }
    }
}
