use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 滞留药水溅射时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct LingeringPotionSplashEvent {
    /// 药水实体的 ID。
    pub entity_id: i32,
    /// 溅射发生的位置。
    pub location: BlockPos,
    /// 药水物品名称。
    pub potion_item: String,
}

impl LingeringPotionSplashEvent {
    #[must_use]
    pub const fn new(entity_id: i32, location: BlockPos, potion_item: String) -> Self {
        Self {
            entity_id,
            location,
            potion_item,
            cancelled: false,
        }
    }
}
