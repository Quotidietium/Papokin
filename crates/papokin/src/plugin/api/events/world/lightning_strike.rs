use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 世界中落雷时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct LightningStrikeEvent {
    /// 雷击的位置。
    pub position: Vector3<f64>,

    /// 此次雷击是否由效果生成。
    pub is_effect: bool,
}

impl LightningStrikeEvent {
    #[must_use]
    pub const fn new(position: Vector3<f64>, is_effect: bool) -> Self {
        Self {
            position,
            is_effect,
            cancelled: false,
        }
    }
}
