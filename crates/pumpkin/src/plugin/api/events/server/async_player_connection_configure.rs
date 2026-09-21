use pumpkin_macros::Event;
use uuid::Uuid;

/// An event that occurs asynchronously when a player connection is being
/// configured (at the start of the configuration phase).
#[derive(Event, Clone)]
pub struct AsyncPlayerConnectionConfigureEvent {
    /// The name of the connecting player.
    pub player_name: String,

    /// The UUID of the connecting player.
    pub player_uuid: Uuid,

    /// Whether the player joins for the first time.
    pub first_join: bool,
}

impl AsyncPlayerConnectionConfigureEvent {
    #[must_use]
    pub const fn new(player_name: String, player_uuid: Uuid, first_join: bool) -> Self {
        Self {
            player_name,
            player_uuid,
            first_join,
        }
    }
}
