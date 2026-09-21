use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a cooldown is applied to an item cooldown group
/// for a player.
///
/// Fires alongside (after) [`super::player_item_cooldown::PlayerItemCooldownEvent`]
/// whenever a cooldown is applied. Cancelling prevents the cooldown from being
/// applied.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemGroupCooldownEvent {
    /// The player receiving the cooldown.
    pub player: Arc<Player>,

    /// The identifier of the cooldown group.
    pub cooldown_group: String,

    /// The cooldown duration in ticks.
    pub cooldown: i32,
}

impl PlayerItemGroupCooldownEvent {
    /// Creates a new instance of `PlayerItemGroupCooldownEvent`.
    pub fn new(player: Arc<Player>, cooldown_group: impl Into<String>, cooldown: i32) -> Self {
        Self {
            player,
            cooldown_group: cooldown_group.into(),
            cooldown,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemGroupCooldownEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
