use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// The action performed on an item frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemFrameAction {
    /// An item is placed into the frame.
    Place,

    /// The item is removed from the frame.
    Remove,

    /// The item in the frame is rotated.
    Rotate,
}

/// An event that occurs when a player places, removes or rotates an item in an
/// item frame.
///
/// Cancelling vetoes the change.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemFrameChangeEvent {
    /// The player interacting with the item frame.
    pub player: Arc<Player>,

    /// The entity id of the item frame.
    pub frame_id: i32,

    /// The item involved in the action (the item placed/removed, or the item
    /// currently displayed when rotating).
    pub item: ItemStack,

    /// The action performed on the item frame.
    pub action: ItemFrameAction,
}

impl PlayerItemFrameChangeEvent {
    /// Creates a new instance of `PlayerItemFrameChangeEvent`.
    pub const fn new(
        player: Arc<Player>,
        frame_id: i32,
        item: ItemStack,
        action: ItemFrameAction,
    ) -> Self {
        Self {
            player,
            frame_id,
            item,
            action,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemFrameChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
