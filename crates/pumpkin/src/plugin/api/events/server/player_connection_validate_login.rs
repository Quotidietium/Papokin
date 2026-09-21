use pumpkin_macros::Event;
use pumpkin_util::text::TextComponent;

/// The outcome of a login validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionValidationResult {
    /// The login is allowed to proceed.
    Allowed,
    /// The login is denied; the client is kicked with the kick message.
    Denied,
}

/// An event that occurs when an incoming login is validated, allowing it to be
/// denied early or a denial to be overridden.
///
/// The event is fired with the vanilla validation verdict; handlers may change
/// [`Self::result`] to override it and [`Self::kick_message`] to customize the
/// disconnect message shown when the login is denied.
#[derive(Event, Clone)]
pub struct PlayerConnectionValidateLoginEvent {
    /// The IP address of the connecting client.
    pub ip_address: String,

    /// The kick message sent when the login is denied.
    pub kick_message: TextComponent,

    /// The validation result.
    pub result: ConnectionValidationResult,
}

impl PlayerConnectionValidateLoginEvent {
    #[must_use]
    pub const fn new(
        ip_address: String,
        kick_message: TextComponent,
        result: ConnectionValidationResult,
    ) -> Self {
        Self {
            ip_address,
            kick_message,
            result,
        }
    }
}
