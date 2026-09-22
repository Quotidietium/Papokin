use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 运输物品的实体（如漏斗）移动物品时发生的事件
/// 矿车）会校验其从中抽取物品的目标方块。
#[cancellable]
#[derive(Event, Clone)]
pub struct ItemTransportingEntityValidateTargetEvent {
    /// 运载实体的 ID。
    pub entity_id: i32,

    /// 正在校验的目标方块的位置。
    pub target_pos: BlockPos,
}

impl ItemTransportingEntityValidateTargetEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_pos: BlockPos) -> Self {
        Self {
            entity_id,
            target_pos,
            cancelled: false,
        }
    }
}
