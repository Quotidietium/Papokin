use pumpkin_macros::Event;
use uuid::Uuid;

/// The kind of whitelist state update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhitelistStateUpdateStatus {
    /// The player was added to the whitelist.
    Added,
    /// The player was removed from the whitelist.
    Removed,
}

/// An event that occurs when the whitelist state of a player is updated.
#[derive(Event, Clone)]
pub struct WhitelistStateUpdateEvent {
    /// The name of the target player.
    pub player_name: String,

    /// The UUID of the target player.
    pub player_uuid: Uuid,

    /// The new whitelist status of the target.
    pub status: WhitelistStateUpdateStatus,
}

impl WhitelistStateUpdateEvent {
    #[must_use]
    pub const fn new(
        player_name: String,
        player_uuid: Uuid,
        status: WhitelistStateUpdateStatus,
    ) -> Self {
        Self {
            player_name,
            player_uuid,
            status,
        }
    }
}
