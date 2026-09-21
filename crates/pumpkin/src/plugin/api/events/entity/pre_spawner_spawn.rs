use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;

/// An event that occurs before a spawner spawns an entity.
#[cancellable]
#[derive(Event, Clone)]
pub struct PreSpawnerSpawnEvent {
    /// Position of the spawner block.
    pub spawner_pos: BlockPos,

    /// The identifier of the entity type being spawned (e.g. `minecraft:zombie`).
    pub entity_type: String,
}

impl PreSpawnerSpawnEvent {
    #[must_use]
    pub const fn new(spawner_pos: BlockPos, entity_type: String) -> Self {
        Self {
            spawner_pos,
            entity_type,
            cancelled: false,
        }
    }
}
