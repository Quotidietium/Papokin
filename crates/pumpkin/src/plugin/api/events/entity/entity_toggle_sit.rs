use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an entity toggles its sitting state.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityToggleSitEvent {
    /// The ID of the entity toggling its sitting state.
    pub entity_id: i32,

    /// Whether the entity is now sitting.
    pub sitting: bool,
}

impl EntityToggleSitEvent {
    #[must_use]
    pub const fn new(entity_id: i32, sitting: bool) -> Self {
        Self {
            entity_id,
            sitting,
            cancelled: false,
        }
    }
}
