use papokin_macros::{Event, cancellable};

/// 载具被摧毁时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleDestroyEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 攻击者的 ID（如适用）。
    pub attacker_id: Option<i32>,
}

impl VehicleDestroyEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, attacker_id: Option<i32>) -> Self {
        Self {
            vehicle_id,
            attacker_id,
            cancelled: false,
        }
    }
}
