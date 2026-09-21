use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when an entity stops being tracked by a player (the
/// entity is destroyed client-side for that player).
///
/// Pure notification; to veto visibility changes use
/// [`super::player_hide_entity::PlayerHideEntityEvent`] instead.
#[derive(Event, Clone)]
pub struct PlayerUntrackEntityEvent {
    /// The player stopping tracking the entity.
    pub player: Arc<Player>,

    /// The entity id of the entity no longer tracked.
    pub entity_id: i32,
}

impl PlayerUntrackEntityEvent {
    /// Creates a new instance of `PlayerUntrackEntityEvent`.
    pub const fn new(player: Arc<Player>, entity_id: i32) -> Self {
        Self { player, entity_id }
    }
}

impl PlayerEvent for PlayerUntrackEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
