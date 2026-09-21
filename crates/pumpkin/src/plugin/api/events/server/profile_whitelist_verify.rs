use pumpkin_macros::Event;
use pumpkin_util::text::TextComponent;
use uuid::Uuid;

/// The outcome of a whitelist verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhitelistVerifyResult {
    /// The player is allowed to join.
    Allowed,
    /// The player is denied; the client is kicked with the kick message.
    Denied,
}

/// An event that occurs when a connecting player is verified against the
/// whitelist.
///
/// The event is fired with the vanilla verdict; handlers may change
/// [`Self::result`] to override it and [`Self::kick_message`] to customize the
/// disconnect message shown when the player is not whitelisted.
#[derive(Event, Clone)]
pub struct ProfileWhitelistVerifyEvent {
    /// The UUID of the connecting player.
    pub player_uuid: Uuid,

    /// The name of the connecting player.
    pub player_name: String,

    /// The kick message sent when the player is not whitelisted.
    pub kick_message: TextComponent,

    /// The verification result.
    pub result: WhitelistVerifyResult,
}

impl ProfileWhitelistVerifyEvent {
    #[must_use]
    pub const fn new(
        player_uuid: Uuid,
        player_name: String,
        kick_message: TextComponent,
        result: WhitelistVerifyResult,
    ) -> Self {
        Self {
            player_uuid,
            player_name,
            kick_message,
            result,
        }
    }
}
