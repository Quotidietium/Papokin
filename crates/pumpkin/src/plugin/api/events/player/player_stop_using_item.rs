use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player stops using an item (e.g. releases a
/// drawn bow).
///
/// Pure notification; the item-use stop itself is not vetoable at this point.
#[derive(Event, Clone)]
pub struct PlayerStopUsingItemEvent {
    /// The player that stopped using the item.
    pub player: Arc<Player>,

    /// The item that was being used.
    pub item: ItemStack,
}

impl PlayerStopUsingItemEvent {
    /// Creates a new instance of `PlayerStopUsingItemEvent`.
    pub const fn new(player: Arc<Player>, item: ItemStack) -> Self {
        Self { player, item }
    }
}

impl PlayerEvent for PlayerStopUsingItemEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
