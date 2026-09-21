use pumpkin_macros::Event;
use uuid::Uuid;

/// The result of a full-server join check.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ServerFullCheckResult {
    /// The player is allowed to join despite the server being full.
    Allowed,

    /// The player is denied and kicked (default).
    #[default]
    Denied,
}

/// An event that occurs when a player tries to join a full server.
///
/// The event is only fired when the server is actually full. Setting
/// `result` to [`ServerFullCheckResult::Allowed`] lets the player join
/// anyway; the default is [`ServerFullCheckResult::Denied`].
///
/// The connection does not have a player object yet, so this event does not
/// implement `PlayerEvent`. This is a Java-protocol login event.
#[derive(Event, Clone)]
pub struct PlayerServerFullCheckEvent {
    /// The name of the joining player.
    pub player_name: String,

    /// The UUID of the joining player.
    pub player_uuid: Uuid,

    /// Whether the player is allowed to join despite the server being full.
    pub result: ServerFullCheckResult,
}

impl PlayerServerFullCheckEvent {
    /// Creates a new instance of `PlayerServerFullCheckEvent`.
    pub fn new(player_name: impl Into<String>, player_uuid: Uuid) -> Self {
        Self {
            player_name: player_name.into(),
            player_uuid,
            result: ServerFullCheckResult::Denied,
        }
    }
}
