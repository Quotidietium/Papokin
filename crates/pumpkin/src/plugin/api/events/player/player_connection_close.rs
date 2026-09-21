use pumpkin_macros::Event;
use uuid::Uuid;

/// An event that occurs when a player connection is closed.
///
/// This is a pure notification fired for every connection teardown,
/// including connections that never finished logging in; therefore it does
/// not carry a player object and does not implement `PlayerEvent`.
#[derive(Event, Clone)]
pub struct PlayerConnectionCloseEvent {
    /// The UUID of the disconnecting player.
    pub player_uuid: Uuid,

    /// The name of the disconnecting player (empty when the connection never
    /// reached the login stage).
    pub player_name: String,

    /// The IP address of the disconnecting player.
    pub ip_address: String,
}

impl PlayerConnectionCloseEvent {
    /// Creates a new instance of `PlayerConnectionCloseEvent`.
    pub fn new(
        player_uuid: Uuid,
        player_name: impl Into<String>,
        ip_address: impl Into<String>,
    ) -> Self {
        Self {
            player_uuid,
            player_name: player_name.into(),
            ip_address: ip_address.into(),
        }
    }
}
