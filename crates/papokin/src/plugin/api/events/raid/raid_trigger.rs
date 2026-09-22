use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 袭击被触发时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct RaidTriggerEvent {
    /// 袭击中心方块位置。
    pub pos: BlockPos,
}

impl RaidTriggerEvent {
    #[must_use]
    pub const fn new(pos: BlockPos) -> Self {
        Self {
            pos,
            cancelled: false,
        }
    }
}
