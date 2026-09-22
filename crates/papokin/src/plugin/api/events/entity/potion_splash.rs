use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 喷溅药水命中并施加效果时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PotionSplashEvent {
    /// 药水实体的 ID。
    pub entity_id: i32,
    /// 闪屏标语（splash）资源位置。
    pub location: BlockPos,
    /// 药水物品名称。
    pub potion_item: String,
    /// 受影响的实体 ID。
    pub affected_entities: Vec<i32>,
}

impl PotionSplashEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        location: BlockPos,
        potion_item: String,
        affected_entities: Vec<i32>,
    ) -> Self {
        Self {
            entity_id,
            location,
            potion_item,
            affected_entities,
            cancelled: false,
        }
    }
}
