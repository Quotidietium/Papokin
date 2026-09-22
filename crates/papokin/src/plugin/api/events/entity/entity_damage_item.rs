use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};

/// 实体手持或穿戴的物品受到耐久损耗时发生的事件
/// 耐久度伤害。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDamageItemEvent {
    /// 物品受损的实体 ID。
    pub entity_id: i32,

    /// 正在受损的物品。
    pub item: ItemStack,

    /// 对物品造成的耐久损耗量。
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
