use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 世界中触发通用游戏事件时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct GenericGameEvent {
    /// 游戏事件的键或类型。
    pub event_key: String,

    /// 游戏事件发生的位置。
    pub position: Vector3<f64>,
}

impl GenericGameEvent {
    #[must_use]
    pub const fn new(event_key: String, position: Vector3<f64>) -> Self {
        Self {
            event_key,
            position,
            cancelled: false,
        }
    }
}
