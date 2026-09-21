use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a cooldown is applied to an item type for a
/// player.
///
/// In this implementation item cooldowns are keyed by cooldown group; this
/// event fires when the cooldown key names a concrete item type (the default
/// when no explicit cooldown group is set on the item). Cancelling prevents
/// the cooldown from being applied.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemCooldownEvent {
    /// The player receiving the cooldown.
    pub player: Arc<Player>,

    /// The identifier of the item type (e.g. `minecraft:ender_pearl`).
    pub item_type: String,

    /// The cooldown duration in ticks.
    pub cooldown: i32,
}

impl PlayerItemCooldownEvent {
    /// Creates a new instance of `PlayerItemCooldownEvent`.
    pub fn new(player: Arc<Player>, item_type: impl Into<String>, cooldown: i32) -> Self {
        Self {
            player,
            item_type: item_type.into(),
            cooldown,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemCooldownEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
