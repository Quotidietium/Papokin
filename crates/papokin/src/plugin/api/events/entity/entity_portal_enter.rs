use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体进入传送门时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPortalEnterEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 进入的传送门方块的位置。
    pub location: BlockPos,
}

impl EntityPortalEnterEvent {
    #[must_use]
    pub const fn new(entity_id: i32, location: BlockPos) -> Self {
        Self {
            entity_id,
            location,
            cancelled: false,
        }
    }
}
