use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 实体爆炸时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityExplodeEvent {
    /// 爆炸实体的 ID。
    pub entity_id: i32,

    /// 爆炸发生的位置。
    pub position: Vector3<f64>,

    /// 方块被破坏时的产出率。
    pub yield_rate: f32,
}

impl EntityExplodeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, position: Vector3<f64>, yield_rate: f32) -> Self {
        Self {
            entity_id,
            position,
            yield_rate,
            cancelled: false,
        }
    }
}
