use pumpkin_macros::Event;

/// An event that occurs when an entity jumps.
#[derive(Event, Clone)]
pub struct EntityJumpEvent {
    /// The ID of the entity that jumped.
    pub entity_id: i32,
}

impl EntityJumpEvent {
    #[must_use]
    pub const fn new(entity_id: i32) -> Self {
        Self { entity_id }
    }
}
