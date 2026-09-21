use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when an item in a player's inventory slot changes.
///
/// This is a high-frequency notification event fired from the screen handler
/// slot tracker. The slot index is the raw slot index within the currently
/// tracked screen handler (player inventory slots follow the container slots).
/// `old_item` reflects the last client-synced state of the slot and may be
/// `None` when no previous state is known.
#[derive(Event, Clone)]
pub struct PlayerInventorySlotChangeEvent {
    /// The player whose inventory changed.
    pub player: Arc<Player>,

    /// The inventory slot that changed.
    pub slot: i32,

    /// The item previously in the slot, if known.
    pub old_item: Option<ItemStack>,

    /// The item now in the slot, if any.
    pub new_item: Option<ItemStack>,
}

impl PlayerInventorySlotChangeEvent {
    /// Creates a new instance of `PlayerInventorySlotChangeEvent`.
    pub const fn new(
        player: Arc<Player>,
        slot: i32,
        old_item: Option<ItemStack>,
        new_item: Option<ItemStack>,
    ) -> Self {
        Self {
            player,
            slot,
            old_item,
            new_item,
        }
    }
}

impl PlayerEvent for PlayerInventorySlotChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
