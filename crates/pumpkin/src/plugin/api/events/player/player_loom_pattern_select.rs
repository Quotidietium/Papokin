use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player selects a pattern in a loom.
///
/// Cancelling vetoes the selection. This is fired on the Java container-button
/// path; the Bedrock equivalent goes through item-stack requests and is not
/// covered yet.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLoomPatternSelectEvent {
    /// The player selecting the pattern.
    pub player: Arc<Player>,

    /// The identifier of the selected banner pattern (e.g. `minecraft:creeper`).
    pub pattern: String,
}

impl PlayerLoomPatternSelectEvent {
    /// Creates a new instance of `PlayerLoomPatternSelectEvent`.
    pub fn new(player: Arc<Player>, pattern: impl Into<String>) -> Self {
        Self {
            player,
            pattern: pattern.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLoomPatternSelectEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
