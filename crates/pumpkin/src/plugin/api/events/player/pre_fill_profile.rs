use pumpkin_macros::Event;
use uuid::Uuid;

/// An event that occurs before a player profile is filled with its
/// properties, allowing a plugin to supply a cached profile.
///
/// Handlers may populate `properties` (and `player_name`) with cached data;
/// the current host implementation always performs the fetch afterwards and
/// does not short-circuit on a pre-filled profile yet (see the integration
/// notes in the event wiring report). There is no player object at this
/// stage, so this event does not implement `PlayerEvent`.
#[derive(Event, Clone)]
pub struct PreFillProfileEvent {
    /// The UUID of the profile.
    pub player_uuid: Uuid,

    /// The name of the profile, if known.
    pub player_name: Option<String>,

    /// The profile properties as name/value pairs (modifiable).
    pub properties: Vec<(String, String)>,
}

impl PreFillProfileEvent {
    /// Creates a new instance of `PreFillProfileEvent`.
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
