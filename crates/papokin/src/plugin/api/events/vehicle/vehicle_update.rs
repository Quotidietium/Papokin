use papokin_macros::{Event, cancellable};

/// 载具每刻更新时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleUpdateEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
}

impl VehicleUpdateEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32) -> Self {
        Self {
            vehicle_id,
            cancelled: false,
        }
    }
}
