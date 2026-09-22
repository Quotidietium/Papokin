use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 酿造台中的燃料被补充或消耗时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BrewingStandFuelEvent {
    /// 酿造台的方块位置。
    pub block_pos: BlockPos,

    /// 燃料的能量值。
    pub fuel_power: u16,
}

impl BrewingStandFuelEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, fuel_power: u16) -> Self {
        Self {
            block_pos,
            fuel_power,
            cancelled: false,
        }
    }
}
