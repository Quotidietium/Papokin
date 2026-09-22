use crate::entity::player::Player;
use papokin_macros::Event;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 玩家从熔炉输出槽位取出物品时发生的事件。
#[derive(Event, Clone)]
pub struct FurnaceExtractEvent {
    /// 取走物品的玩家。
    pub player: Arc<Player>,

    /// 熔炉方块的位置。
    pub block_pos: BlockPos,

    /// 被取出物品的注册表键。
    pub item_id: String,

    /// 被抽取的物品数量。
    pub item_amount: u32,

    /// 从提取中获得的经验。
    pub exp_gained: f32,
}

impl FurnaceExtractEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        block_pos: BlockPos,
        item_id: String,
        item_amount: u32,
        exp_gained: f32,
    ) -> Self {
        Self {
            player,
            block_pos,
            item_id,
            item_amount,
            exp_gained,
        }
    }
}
