use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player picks an entity (middle click) and
/// receives the corresponding spawn egg.
///
/// Cancelling vetoes the pick. The `result` item is computed by the host;
/// modifications from the guest are currently not applied to the pick logic.
/// This is a Java-protocol event.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickEntityEvent {
    /// The player picking the entity.
    pub player: Arc<Player>,

    /// The entity id of the picked entity.
    pub entity_id: i32,

    /// The resulting item given to the player.
    pub result: ItemStack,
}

impl PlayerPickEntityEvent {
    /// Creates a new instance of `PlayerPickEntityEvent`.
    pub const fn new(player: Arc<Player>, entity_id: i32, result: ItemStack) -> Self {
        Self {
            player,
            entity_id,
            result,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
