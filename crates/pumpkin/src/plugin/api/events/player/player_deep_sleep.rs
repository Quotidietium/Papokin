use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player enters deep sleep (100 ticks of sleep,
/// the point where vanilla starts counting the player towards the sleep
/// percentage and phantom spawning is suppressed).
///
/// Pure notification; fired once per sleep cycle.
#[derive(Event, Clone)]
pub struct PlayerDeepSleepEvent {
    /// The player entering deep sleep.
    pub player: Arc<Player>,
}

impl PlayerDeepSleepEvent {
    /// Creates a new instance of `PlayerDeepSleepEvent`.
    pub const fn new(player: Arc<Player>) -> Self {
        Self { player }
    }
}

impl PlayerEvent for PlayerDeepSleepEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
