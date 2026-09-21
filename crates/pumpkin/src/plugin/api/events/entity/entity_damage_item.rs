use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an item held or worn by an entity takes
/// durability damage.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDamageItemEvent {
    /// The ID of the entity whose item is damaged.
    pub entity_id: i32,

    /// The item being damaged.
    pub item: ItemStack,

    /// The amount of durability damage dealt to the item.
    pub damage: i32,
}

impl EntityDamageItemEvent {
    #[must_use]
    pub const fn new(entity_id: i32, item: ItemStack, damage: i32) -> Self {
        Self {
            entity_id,
            item,
            damage,
            cancelled: false,
        }
    }
}
