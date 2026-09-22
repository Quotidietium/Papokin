use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体离开传送门时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPortalExitEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 传送门来源位置。
    pub from_pos: BlockPos,
    /// 目标位置。
    pub to_pos: Option<BlockPos>,
}

impl EntityPortalExitEvent {
    #[must_use]
    pub const fn new(entity_id: i32, from_pos: BlockPos, to_pos: Option<BlockPos>) -> Self {
        Self {
            entity_id,
            from_pos,
            to_pos,
            cancelled: false,
        }
    }
}
