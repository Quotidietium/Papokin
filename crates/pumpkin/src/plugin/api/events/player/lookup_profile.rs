use pumpkin_macros::Event;
use uuid::Uuid;

/// An event that occurs when a player profile has been looked up by name.
///
/// `properties` is currently always empty: the name lookup only resolves the
/// UUID, properties are fetched later by the fill step. This is a pure
/// notification and does not implement `PlayerEvent`.
#[derive(Event, Clone)]
pub struct LookupProfileEvent {
    /// The name that was looked up.
    pub name: String,

    /// The UUID of the resolved profile.
    pub player_uuid: Uuid,

    /// The name of the resolved profile, if any.
    pub player_name: Option<String>,

    /// The profile properties as name/value pairs (currently always empty).
    pub properties: Vec<(String, String)>,
}

impl LookupProfileEvent {
    /// Creates a new instance of `LookupProfileEvent`.
    pub fn new(
        name: impl Into<String>,
        player_uuid: Uuid,
        player_name: Option<String>,
        properties: Vec<(String, String)>,
    ) -> Self {
        Self {
            name: name.into(),
            player_uuid,
            player_name,
            properties,
        }
    }
}
