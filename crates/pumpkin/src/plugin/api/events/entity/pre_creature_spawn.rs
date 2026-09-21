use std::sync::Arc;

use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::vector3::Vector3;

use crate::world::World;

/// An event that occurs before a creature spawns, allowing early filtering.
#[cancellable]
#[derive(Event, Clone)]
pub struct PreCreatureSpawnEvent {
    /// The spawn position.
    pub position: Vector3<f64>,

    /// The world the creature spawns in.
    pub world: Arc<World>,

    /// The identifier of the entity type being spawned (e.g. `minecraft:zombie`).
    pub entity_type: String,

    /// The spawn reason (e.g. `NATURAL`, `CHUNK_GENERATION`).
    pub reason: String,
}

impl PreCreatureSpawnEvent {
    #[must_use]
    pub const fn new(
        position: Vector3<f64>,
        world: Arc<World>,
        entity_type: String,
        reason: String,
    ) -> Self {
        Self {
            position,
            world,
            entity_type,
            reason,
            cancelled: false,
        }
    }
}
