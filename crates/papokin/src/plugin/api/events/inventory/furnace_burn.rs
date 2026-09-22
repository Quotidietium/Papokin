use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 物品在熔炉中作为燃料被消耗时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct FurnaceBurnEvent {
    /// 熔炉方块的位置。
    pub block_pos: BlockPos,

    /// 燃料物品的注册表键。
    pub fuel_item: String,

    /// 燃烧时间（以刻为单位）。
    pub burn_time: u32,
}

impl FurnaceBurnEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, fuel_item: String, burn_time: u32) -> Self {
        Self {
            block_pos,
            fuel_item,
            burn_time,
            cancelled: false,
        }
    }
}
