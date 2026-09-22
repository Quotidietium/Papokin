use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};

/// 女巫向目标投掷喷溅药水时发生的事件。
///
/// 取消会完全阻止投掷（不会生成投掷物）。
#[cancellable]
#[derive(Event, Clone)]
pub struct WitchThrowPotionEvent {
    /// 正在投掷药水的女巫的 ID。
    pub entity_id: i32,

    /// 正在投掷的喷溅型药水。
    pub potion: ItemStack,

    /// 药水掷向的实体 ID（如有）。
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
