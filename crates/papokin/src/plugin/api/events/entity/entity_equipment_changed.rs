use papokin_data::item_stack::ItemStack;
use papokin_macros::Event;

/// 实体某个槽位中的装备变化时发生的事件。
#[derive(Event, Clone)]
pub struct EntityEquipmentChangedEvent {
    /// 装备发生变化的实体 ID。
    pub entity_id: i32,

    /// 发生变化的装备槽位（例如 `mainhand`）。
    pub slot: String,

    /// 之前位于槽位中的物品，如果已知的话。
    pub old_item: Option<ItemStack>,

    /// 现在位于槽位中的物品，如果有的话。
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
