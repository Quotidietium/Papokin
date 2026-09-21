use pumpkin_macros::{Event, cancellable};

/// The intention of a handshake.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandshakeIntention {
    /// The client requests the server status (server list ping).
    Status,

    /// The client wants to log in.
    Login,
}

/// An event that occurs when a client sends a handshake packet.
///
/// Cancelling immediately disconnects the client without further processing.
///
/// The connection does not have a player object yet, so this event does not
/// implement `PlayerEvent`. This is a Java-protocol event.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerHandshakeEvent {
    /// The IP address of the client.
    pub ip_address: String,

    /// The hostname the client used to connect.
    pub hostname: String,

    /// The protocol version of the client.
    pub protocol_version: i32,

    /// The intention of the handshake (status or login).
    pub intention: HandshakeIntention,
}

impl PlayerHandshakeEvent {
    /// Creates a new instance of `PlayerHandshakeEvent`.
    pub fn new(
        ip_address: impl Into<String>,
        hostname: impl Into<String>,
        protocol_version: i32,
        intention: HandshakeIntention,
    ) -> Self {
        Self {
            ip_address: ip_address.into(),
            hostname: hostname.into(),
            protocol_version,
            intention,
            cancelled: false,
        }
    }
}
