use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player completes a purchase with a merchant.
///
/// Cancelling vetoes the trade. `merchant_id` is `None` when the merchant
/// screen is not backed by a living merchant entity (e.g. a custom merchant
/// menu); the id of the merchant entity otherwise.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPurchaseEvent {
    /// The player making the purchase.
    pub player: Arc<Player>,

    /// The merchant entity id, if any.
    pub merchant_id: Option<i32>,

    /// The ingredient items of the trade.
    pub ingredients: Vec<ItemStack>,

    /// The resulting item of the trade.
    pub result: ItemStack,
}

impl PlayerPurchaseEvent {
    /// Creates a new instance of `PlayerPurchaseEvent`.
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        merchant_id: Option<i32>,
        ingredients: Vec<ItemStack>,
        result: ItemStack,
    ) -> Self {
        Self {
            player,
            merchant_id,
            ingredients,
            result,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPurchaseEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
