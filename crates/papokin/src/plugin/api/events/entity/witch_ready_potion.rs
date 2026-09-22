use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};

/// 女巫举起药水并开始饮用时发生的事件。
///
/// 取消会完全阻止饮用：药水不会被放入
/// 女巫的手中，且不会开始饮用。
#[cancellable]
#[derive(Event, Clone)]
pub struct WitchReadyPotionEvent {
    /// 正在准备药水的女巫的 ID。
    pub entity_id: i32,

    /// 正在准备的药水。
    pub potion: ItemStack,
}

impl WitchReadyPotionEvent {
    #[must_use]
    pub const fn new(entity_id: i32, potion: ItemStack) -> Self {
        Self {
            entity_id,
            potion,
            cancelled: false,
        }
    }
}
