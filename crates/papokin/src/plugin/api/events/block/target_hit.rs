use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 标靶方块被弹射物或实体击中时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct TargetHitEvent {
    /// 命中目标的发射者/所有者的实体 ID（如有）。
    pub shooter_id: Option<i32>,

    /// 目标方块的位置。
    pub block_pos: BlockPos,

    /// 发出的红石信号强度（0-15）。
    pub signal_strength: i32,
}

impl TargetHitEvent {
    #[must_use]
    pub const fn new(shooter_id: Option<i32>, block_pos: BlockPos, signal_strength: i32) -> Self {
        Self {
            shooter_id,
            block_pos,
            signal_strength,
            cancelled: false,
        }
    }
}
