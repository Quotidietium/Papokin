use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 载具移动时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleMoveEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 起始位置。
    pub from: Vector3<f64>,
    /// 目标位置。
    pub to: Vector3<f64>,
}

impl VehicleMoveEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, from: Vector3<f64>, to: Vector3<f64>) -> Self {
        Self {
            vehicle_id,
            from,
            to,
            cancelled: false,
        }
    }
}
