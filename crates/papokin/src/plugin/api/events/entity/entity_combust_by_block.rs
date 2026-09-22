use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体被方块点燃时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityCombustByBlockEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 引燃方块坐标。
    pub combuster: BlockPos,
    /// 燃烧持续时间（秒）。
    pub duration: f32,
}

impl EntityCombustByBlockEvent {
    #[must_use]
    pub const fn new(entity_id: i32, combuster: BlockPos, duration: f32) -> Self {
        Self {
            entity_id,
            combuster,
            duration,
            cancelled: false,
        }
    }
}
