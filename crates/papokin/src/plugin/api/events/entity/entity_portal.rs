use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体进入传送门以在维度之间穿行时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPortalEvent {
    /// 进入传送门的实体 ID。
    pub entity_id: i32,

    /// 传送门方块的位置。
    pub portal_pos: BlockPos,
}

impl EntityPortalEvent {
    #[must_use]
    pub const fn new(entity_id: i32, portal_pos: BlockPos) -> Self {
        Self {
            entity_id,
            portal_pos,
            cancelled: false,
        }
    }
}
