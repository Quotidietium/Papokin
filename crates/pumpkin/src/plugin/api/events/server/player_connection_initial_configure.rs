use pumpkin_macros::Event;
use uuid::Uuid;

/// An event that occurs when a player connection is initially configured
/// (when the configuration phase completes, before the player enters the world).
#[derive(Event, Clone)]
pub struct PlayerConnectionInitialConfigureEvent {
    /// The name of the connecting player.
    pub player_name: String,

    /// The UUID of the connecting player.
    pub player_uuid: Uuid,

    /// Whether the player joins for the first time.
    pub first_join: bool,
}

impl PlayerConnectionInitialConfigureEvent {
    #[must_use]
    pub const fn new(player_name: String, player_uuid: Uuid, first_join: bool) -> Self {
        Self {
            player_name,
            player_uuid,
            first_join,
        }
    }
}
