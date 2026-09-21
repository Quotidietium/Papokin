use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player trades with a villager or wandering
/// trader.
///
/// Cancelling vetoes the trade. `merchant_id` is `-1` when the merchant
/// screen is not backed by a living merchant entity (the WIT field is a plain
/// `s32`, so a sentinel is used instead of an option).
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerTradeEvent {
    /// The player trading.
    pub player: Arc<Player>,

    /// The merchant entity id (`-1` when there is no merchant entity).
    pub merchant_id: i32,

    /// The ingredient items of the trade.
    pub ingredients: Vec<ItemStack>,

    /// The resulting item of the trade.
    pub result: ItemStack,

    /// The experience the villager gains from the trade.
    pub villager_experience: i32,
}

impl PlayerTradeEvent {
    /// Creates a new instance of `PlayerTradeEvent`.
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        merchant_id: i32,
        ingredients: Vec<ItemStack>,
        result: ItemStack,
        villager_experience: i32,
    ) -> Self {
        Self {
            player,
            merchant_id,
            ingredients,
            result,
            villager_experience,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerTradeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
