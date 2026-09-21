use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};

/// An event that occurs when a witch throws a splash potion at its target.
///
/// Cancelling prevents the throw entirely (no projectile is spawned).
#[cancellable]
#[derive(Event, Clone)]
pub struct WitchThrowPotionEvent {
    /// The ID of the witch throwing the potion.
    pub entity_id: i32,

    /// The splash potion being thrown.
    pub potion: ItemStack,

    /// The ID of the entity the potion is thrown at, if any.
    pub target_id: Option<i32>,
}

impl WitchThrowPotionEvent {
    #[must_use]
    pub const fn new(entity_id: i32, potion: ItemStack, target_id: Option<i32>) -> Self {
        Self {
            entity_id,
            potion,
            target_id,
            cancelled: false,
        }
    }
}
