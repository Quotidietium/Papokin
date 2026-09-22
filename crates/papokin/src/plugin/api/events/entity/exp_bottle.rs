use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 经验瓶破裂时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ExpBottleEvent {
    /// 瓶子实体 ID。
    pub entity_id: i32,
    /// 经验数量。
    pub experience: i32,
    /// 瓶子碎裂的位置。
    pub location: BlockPos,
    /// 是否显示粒子效果。
    pub show_effect: bool,
}

impl ExpBottleEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        experience: i32,
        location: BlockPos,
        show_effect: bool,
    ) -> Self {
        Self {
            entity_id,
            experience,
            location,
            show_effect,
            cancelled: false,
        }
    }
}
