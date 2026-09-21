use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};

/// An event that occurs when a witch finishes drinking a potion.
///
/// Cancelling prevents the potion effects from being applied; the potion is
/// still consumed (the witch's main hand is emptied either way).
#[cancellable]
#[derive(Event, Clone)]
pub struct WitchConsumePotionEvent {
    /// The ID of the witch drinking the potion.
    pub entity_id: i32,

    /// The potion being consumed.
    pub potion: ItemStack,
}

impl WitchConsumePotionEvent {
    #[must_use]
    pub const fn new(entity_id: i32, potion: ItemStack) -> Self {
        Self {
            entity_id,
            potion,
            cancelled: false,
        }
    }
}
