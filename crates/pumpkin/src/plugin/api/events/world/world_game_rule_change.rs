use crate::world::World;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

/// An event that occurs when a game rule value changes in a world.
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldGameRuleChangeEvent {
    /// The world the game rule changed in.
    pub world: Arc<World>,

    /// The name of the game rule.
    pub rule: String,

    /// The new value of the game rule.
    pub value: String,
}

impl WorldGameRuleChangeEvent {
    #[must_use]
    pub const fn new(world: Arc<World>, rule: String, value: String) -> Self {
        Self {
            world,
            rule,
            value,
            cancelled: false,
        }
    }
}
