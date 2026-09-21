use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::Event;

/// An event that occurs when an entity's equipment in a slot changes.
#[derive(Event, Clone)]
pub struct EntityEquipmentChangedEvent {
    /// The ID of the entity whose equipment changed.
    pub entity_id: i32,

    /// The equipment slot that changed (e.g. `mainhand`).
    pub slot: String,

    /// The item previously in the slot, if known.
    pub old_item: Option<ItemStack>,

    /// The item now in the slot, if any.
    pub new_item: Option<ItemStack>,
}

impl EntityEquipmentChangedEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        slot: String,
        old_item: Option<ItemStack>,
        new_item: Option<ItemStack>,
    ) -> Self {
        Self {
            entity_id,
            slot,
            old_item,
            new_item,
        }
    }
}
