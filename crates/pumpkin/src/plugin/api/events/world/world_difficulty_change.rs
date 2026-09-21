use crate::world::World;
use pumpkin_macros::Event;
use std::sync::Arc;

/// An event that occurs when the difficulty of a world changes.
#[derive(Event, Clone)]
pub struct WorldDifficultyChangeEvent {
    /// The world whose difficulty changed.
    pub world: Arc<World>,

    /// The previous difficulty name.
    pub old_difficulty: String,

    /// The new difficulty name.
    pub new_difficulty: String,
}

impl WorldDifficultyChangeEvent {
    #[must_use]
    pub const fn new(world: Arc<World>, old_difficulty: String, new_difficulty: String) -> Self {
        Self {
            world,
            old_difficulty,
            new_difficulty,
        }
    }
}
