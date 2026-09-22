use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};

/// 实体为弩装填弹射物时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityLoadCrossbowEvent {
    /// 正在装填弩的实体 ID。
    pub entity_id: i32,

    /// 正在装填的弩。
    pub crossbow: ItemStack,

    /// 正在装填的投射物物品。
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
