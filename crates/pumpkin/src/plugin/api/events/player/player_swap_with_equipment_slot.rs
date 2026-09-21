use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player swaps an item with an armor slot.
///
/// Fired on the player's own inventory screen when clicking an armor slot
/// with an equippable item on the cursor, or using a hotbar-swap key on an
/// armor slot. Cancelling vetoes the swap.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSwapWithEquipmentSlotEvent {
    /// The player swapping the item.
    pub player: Arc<Player>,

    /// The equipment slot being swapped with (e.g. `head`, `chest`, `legs`,
    /// `feet`).
    pub slot: String,

    /// The item currently equipped in the slot, if any.
    pub equipped_item: Option<ItemStack>,

    /// The item being equipped (cursor or hotbar source), if any.
    pub cursor_item: Option<ItemStack>,
}

impl PlayerSwapWithEquipmentSlotEvent {
    /// Creates a new instance of `PlayerSwapWithEquipmentSlotEvent`.
    pub fn new(
        player: Arc<Player>,
        slot: impl Into<String>,
        equipped_item: Option<ItemStack>,
        cursor_item: Option<ItemStack>,
    ) -> Self {
        Self {
            player,
            slot: slot.into(),
            equipped_item,
            cursor_item,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerSwapWithEquipmentSlotEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
