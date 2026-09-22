use crate::world::World;
use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

/// 世界边界的范围（直径）变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldBorderBoundsChangeEvent {
    /// 边界发生变化的世界。
    pub world: Arc<World>,

    /// 原直径（以方块为单位）。
    pub old_diameter: f64,

    /// 以方块为单位的新直径。
    pub new_diameter: f64,

    /// 调整大小所花费的时间（毫秒）。
    pub duration_ms: u64,
}

impl WorldBorderBoundsChangeEvent {
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        old_diameter: f64,
        new_diameter: f64,
        duration_ms: u64,
    ) -> Self {
        Self {
            world,
            old_diameter,
            new_diameter,
            duration_ms,
            cancelled: false,
        }
    }
}

/// 世界边界的中心变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldBorderCenterChangeEvent {
    /// 边界中心发生变化的世界。
    pub world: Arc<World>,

    /// 原中心位置。
    pub old_center: Vector3<f64>,

    /// 新的中心位置。
    pub new_center: Vector3<f64>,
}

impl WorldBorderCenterChangeEvent {
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        old_center: Vector3<f64>,
        new_center: Vector3<f64>,
    ) -> Self {
        Self {
            world,
            old_center,
            new_center,
            cancelled: false,
        }
    }
}
