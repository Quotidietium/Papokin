use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player selects a recipe in a stonecutter.
///
/// Cancelling vetoes the selection. `recipe_id` carries the result item's
/// identifier (e.g. `minecraft:stone_brick_slab`): stonecutting recipes in
/// this engine have no separate registry id of their own. This is a
/// Java-protocol event (container button click).
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStonecutterRecipeSelectEvent {
    /// The player selecting the recipe.
    pub player: Arc<Player>,

    /// The identifier of the selected recipe.
    pub recipe_id: String,
}

impl PlayerStonecutterRecipeSelectEvent {
    /// Creates a new instance of `PlayerStonecutterRecipeSelectEvent`.
    pub fn new(player: Arc<Player>, recipe_id: impl Into<String>) -> Self {
        Self {
            player,
            recipe_id: recipe_id.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerStonecutterRecipeSelectEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
