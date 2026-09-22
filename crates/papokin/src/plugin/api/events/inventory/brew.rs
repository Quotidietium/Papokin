use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 酿造台完成药水酿造时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BrewEvent {
    /// 酿造台的方块位置。
    pub block_pos: BlockPos,

    /// 剩余燃料能量。
    pub fuel_level: u8,
}

impl BrewEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, fuel_level: u8) -> Self {
        Self {
            block_pos,
            fuel_level,
            cancelled: false,
        }
    }
}
