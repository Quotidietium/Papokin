use papokin_macros::{Event, cancellable};

/// 实体进入载具时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleEnterEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 进入的实体 ID。
    pub entered_id: i32,
}

impl VehicleEnterEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, entered_id: i32) -> Self {
        Self {
            vehicle_id,
            entered_id,
            cancelled: false,
        }
    }
}
