use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player readies an arrow with a bow (starts
/// drawing the bow).
///
/// Cancelling prevents the draw from starting.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerReadyArrowEvent {
    /// The player readying the arrow.
    pub player: Arc<Player>,

    /// The bow being used.
    pub bow: ItemStack,

    /// The arrow being readied (the projectile that would be consumed).
    pub arrow: ItemStack,
}

impl PlayerReadyArrowEvent {
    /// Creates a new instance of `PlayerReadyArrowEvent`.
    pub const fn new(player: Arc<Player>, bow: ItemStack, arrow: ItemStack) -> Self {
        Self {
            player,
            bow,
            arrow,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerReadyArrowEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
