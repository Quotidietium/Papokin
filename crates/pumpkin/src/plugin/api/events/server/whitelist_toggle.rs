use pumpkin_macros::Event;

/// An event that occurs when the whitelist is toggled on or off.
#[derive(Event, Clone)]
pub struct WhitelistToggleEvent {
    /// Whether the whitelist is now enabled.
    pub enabled: bool,
}

impl WhitelistToggleEvent {
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}
