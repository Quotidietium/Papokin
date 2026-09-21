use crate::world::World;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

/// An event that occurs when the bounds (diameter) of a world border change.
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldBorderBoundsChangeEvent {
    /// The world whose border changed.
    pub world: Arc<World>,

    /// The old diameter in blocks.
    pub old_diameter: f64,

    /// The new diameter in blocks.
    pub new_diameter: f64,

    /// The time in milliseconds the resize takes.
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

/// An event that occurs when the center of a world border changes.
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldBorderCenterChangeEvent {
    /// The world whose border center changed.
    pub world: Arc<World>,

    /// The old center position.
    pub old_center: Vector3<f64>,

    /// The new center position.
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
