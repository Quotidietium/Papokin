use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

/// An event that occurs asynchronously when tab completions are computed for
/// a command buffer.
///
/// Cancelling suppresses the completions; `completions` may be modified by
/// handlers to add or remove entries. `sender` is `None` when the completion
/// request does not come from a player (e.g. the console or a command block
/// on Bedrock), so this event does not implement `PlayerEvent`.
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncTabCompleteEvent {
    /// The sender requesting completions, if it is a player.
    pub sender: Option<Arc<Player>>,

    /// The current command buffer.
    pub buffer: String,

    /// The computed completions (modifiable).
    pub completions: Vec<String>,
}

impl AsyncTabCompleteEvent {
    /// Creates a new instance of `AsyncTabCompleteEvent`.
    pub fn new(
        sender: Option<Arc<Player>>,
        buffer: impl Into<String>,
        completions: Vec<String>,
    ) -> Self {
        Self {
            sender,
            buffer: buffer.into(),
            completions,
            cancelled: false,
        }
    }
}
