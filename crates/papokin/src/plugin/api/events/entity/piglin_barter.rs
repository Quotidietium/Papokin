use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};

/// 猪灵进行以物易物时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PiglinBarterEvent {
    /// 猪灵实体的 ID。
    pub entity_id: i32,

    /// 给予猪灵的物品。
    pub input_item: ItemStack,

    /// 以物易物产生的结果物品堆。
    pub outcome: Vec<ItemStack>,
}

impl PiglinBarterEvent {
    #[must_use]
    pub const fn new(entity_id: i32, input_item: ItemStack, outcome: Vec<ItemStack>) -> Self {
        Self {
            cancelled: false,
            entity_id,
            input_item,
            outcome,
        }
    }
}
