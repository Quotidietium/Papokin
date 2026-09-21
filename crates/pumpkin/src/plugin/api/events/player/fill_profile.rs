use pumpkin_macros::Event;
use uuid::Uuid;

/// An event that occurs when a player profile has been filled with its
/// properties (fetched from the authentication servers).
///
/// This is a pure notification; there is no player object yet, so this event
/// does not implement `PlayerEvent`.
#[derive(Event, Clone)]
pub struct FillProfileEvent {
    /// The UUID of the profile.
    pub player_uuid: Uuid,

    /// The name of the profile, if resolved.
    pub player_name: Option<String>,

    /// The profile properties as name/value pairs (e.g. `textures`).
    pub properties: Vec<(String, String)>,
}

impl FillProfileEvent {
    /// Creates a new instance of `FillProfileEvent`.
    #[must_use]
    pub const fn new(
        player_uuid: Uuid,
        player_name: Option<String>,
        properties: Vec<(String, String)>,
    ) -> Self {
        Self {
            player_uuid,
            player_name,
            properties,
        }
    }
}
