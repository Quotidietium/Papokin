use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs asynchronously when command suggestions are sent to a
/// player.
///
/// Cancelling suppresses the suggestions; `suggestions` may be modified by
/// handlers to add or remove entries.
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncPlayerSendSuggestionsEvent {
    /// The player receiving the suggestions.
    pub player: Arc<Player>,

    /// The current command buffer.
    pub buffer: String,

    /// The suggestions to send (modifiable).
    pub suggestions: Vec<String>,
}

impl AsyncPlayerSendSuggestionsEvent {
    /// Creates a new instance of `AsyncPlayerSendSuggestionsEvent`.
    pub fn new(player: Arc<Player>, buffer: impl Into<String>, suggestions: Vec<String>) -> Self {
        Self {
            player,
            buffer: buffer.into(),
            suggestions,
            cancelled: false,
        }
    }
}

impl PlayerEvent for AsyncPlayerSendSuggestionsEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
