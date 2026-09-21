use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player's client finishes a tick.
///
/// This is a high-frequency pure notification fired once per client tick
/// packet per player; the zero-listener early-return inside the dispatcher
/// keeps it cheap when no plugin is registered. This is a Java-protocol
/// event.
#[derive(Event, Clone)]
pub struct ClientTickEndEvent {
    /// The player whose client tick ended.
    pub player: Arc<Player>,
}

impl ClientTickEndEvent {
    /// Creates a new instance of `ClientTickEndEvent`.
    pub const fn new(player: Arc<Player>) -> Self {
        Self { player }
    }
}

impl PlayerEvent for ClientTickEndEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
