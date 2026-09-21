use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when an entity starts being tracked (spawned/sent) for
/// a player.
///
/// Pure notification; to veto visibility use
/// [`super::player_show_entity::PlayerShowEntityEvent`] instead.
#[derive(Event, Clone)]
pub struct PlayerTrackEntityEvent {
    /// The player starting to track the entity.
    pub player: Arc<Player>,

    /// The entity id of the entity being tracked.
    pub entity_id: i32,
}

impl PlayerTrackEntityEvent {
    /// Creates a new instance of `PlayerTrackEntityEvent`.
    pub const fn new(player: Arc<Player>, entity_id: i32) -> Self {
        Self { player, entity_id }
    }
}

impl PlayerEvent for PlayerTrackEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
