use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};

/// An event that occurs when a witch raises a potion and starts drinking it.
///
/// Cancelling prevents the drink entirely: the potion is not equipped into
/// the witch's hand and no drinking begins.
#[cancellable]
#[derive(Event, Clone)]
pub struct WitchReadyPotionEvent {
    /// The ID of the witch readying the potion.
    pub entity_id: i32,

    /// The potion being readied.
    pub potion: ItemStack,
}

impl WitchReadyPotionEvent {
    #[must_use]
    pub const fn new(entity_id: i32, potion: ItemStack) -> Self {
        Self {
            entity_id,
            potion,
            cancelled: false,
        }
    }
}
