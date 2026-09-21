use pumpkin_macros::Event;

/// An event that occurs before a player profile is looked up by name,
/// allowing a plugin to supply a cached profile.
///
/// This is a pure notification in the current host implementation; the
/// lookup always proceeds (see the integration notes in the event wiring
/// report). There is no player object at this stage, so this event does not
/// implement `PlayerEvent`.
#[derive(Event, Clone)]
pub struct PreLookupProfileEvent {
    /// The name being looked up.
    pub name: String,
}

impl PreLookupProfileEvent {
    /// Creates a new instance of `PreLookupProfileEvent`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
