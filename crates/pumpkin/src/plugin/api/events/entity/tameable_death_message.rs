use pumpkin_macros::Event;
use pumpkin_util::text::TextComponent;

/// An event that occurs when a tameable entity dies and its death message is
/// created. The message can be replaced by plugins.
#[derive(Event, Clone)]
pub struct TameableDeathMessageEvent {
    /// The ID of the tameable entity that died.
    pub entity_id: i32,

    /// The death message to broadcast.
    pub death_message: TextComponent,
}

impl TameableDeathMessageEvent {
    #[must_use]
    pub const fn new(entity_id: i32, death_message: TextComponent) -> Self {
        Self {
            entity_id,
            death_message,
        }
    }
}
