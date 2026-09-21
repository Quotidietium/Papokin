use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an enderman escapes from combat.
#[cancellable]
#[derive(Event, Clone)]
pub struct EndermanEscapeEvent {
    /// The ID of the escaping enderman.
    pub entity_id: i32,

    /// The escape reason (e.g. `teleport` or `unknown`).
    pub reason: String,
}

impl EndermanEscapeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, reason: String) -> Self {
        Self {
            entity_id,
            reason,
            cancelled: false,
        }
    }
}
