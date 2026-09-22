use papokin_macros::{Event, cancellable};

/// 实体退出载具时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleExitEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 正在离开的实体 ID。
    pub exited_id: i32,
}

impl VehicleExitEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, exited_id: i32) -> Self {
        Self {
            vehicle_id,
            exited_id,
            cancelled: false,
        }
    }
}
