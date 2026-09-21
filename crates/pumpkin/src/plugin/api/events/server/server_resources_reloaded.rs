use pumpkin_macros::Event;

/// An event that occurs when the server resources (data packs) are reloaded.
#[derive(Event, Clone)]
pub struct ServerResourcesReloadedEvent {
    /// The cause of the reload (e.g. command or plugin).
    pub cause: String,
}

impl ServerResourcesReloadedEvent {
    #[must_use]
    pub const fn new(cause: String) -> Self {
        Self { cause }
    }
}
