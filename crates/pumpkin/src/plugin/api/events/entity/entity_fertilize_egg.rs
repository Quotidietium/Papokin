use std::sync::Arc;

use pumpkin_macros::{Event, cancellable};

use crate::entity::player::Player;

/// An event that occurs when an entity (turtle) fertilizes an egg.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityFertilizeEggEvent {
    /// The ID of the entity laying the egg.
    pub entity_id: i32,

    /// The player that bred the entity, if any.
    pub breeder: Option<Arc<Player>>,
}

impl EntityFertilizeEggEvent {
    #[must_use]
    pub const fn new(entity_id: i32, breeder: Option<Arc<Player>>) -> Self {
        Self {
            entity_id,
            breeder,
            cancelled: false,
        }
    }
}
