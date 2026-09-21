use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an entity loads a crossbow with projectiles.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityLoadCrossbowEvent {
    /// The ID of the entity loading the crossbow.
    pub entity_id: i32,

    /// The crossbow being loaded.
    pub crossbow: ItemStack,

    /// The projectile items being loaded.
    pub projectiles: Vec<ItemStack>,
}

impl EntityLoadCrossbowEvent {
    #[must_use]
    pub const fn new(entity_id: i32, crossbow: ItemStack, projectiles: Vec<ItemStack>) -> Self {
        Self {
            entity_id,
            crossbow,
            projectiles,
            cancelled: false,
        }
    }
}
