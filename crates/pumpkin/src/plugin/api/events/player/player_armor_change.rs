use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player's armor piece changes.
///
/// Pure notification fired when an armor equipment change is propagated.
/// `new_item` is the item being equipped (`None` when the piece is removed).
/// `old_item` is only populated when the previous content is still observable
/// at the hook point; it may be `None` otherwise.
#[derive(Event, Clone)]
pub struct PlayerArmorChangeEvent {
    /// The player whose armor changed.
    pub player: Arc<Player>,

    /// The armor slot that changed (`head`, `chest`, `legs` or `feet`).
    pub slot: String,

    /// The previously equipped item, if known.
    pub old_item: Option<ItemStack>,

    /// The newly equipped item, if any.
    pub new_item: Option<ItemStack>,
}

impl PlayerArmorChangeEvent {
    /// Creates a new instance of `PlayerArmorChangeEvent`.
    pub fn new(
        player: Arc<Player>,
        slot: impl Into<String>,
        old_item: Option<ItemStack>,
        new_item: Option<ItemStack>,
    ) -> Self {
        Self {
            player,
            slot: slot.into(),
            old_item,
            new_item,
        }
    }
}

impl PlayerEvent for PlayerArmorChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
