use crate::world::World;
use papokin_macros::Event;
use std::sync::Arc;

/// 世界的难度变化时发生的事件。
#[derive(Event, Clone)]
pub struct WorldDifficultyChangeEvent {
    /// 难度发生变化的世界。
    pub world: Arc<World>,

    /// 之前的难度名称。
    pub old_difficulty: String,

    /// 新的难度名称。
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
