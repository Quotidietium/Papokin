use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};

/// 女巫喝完药水时发生的事件。
///
/// 取消会阻止药水效果被应用；药水
/// 仍会被消耗（无论哪种情况，女巫的主手都会被清空）。
#[cancellable]
#[derive(Event, Clone)]
pub struct WitchConsumePotionEvent {
    /// 正在喝药水的女巫的 ID。
    pub entity_id: i32,

    /// 正被饮用的药水。
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
