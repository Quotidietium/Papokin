use std::sync::Arc;

use papokin_data::Block;
use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

use crate::world::World;

use super::BlockEvent;

/// 方块生长时发生的事件。
///
/// 范围：
/// - 在农作物随机刻生长时触发。
/// - 尚未针对骨粉催熟、树苗/树木生长、海带/仙人掌/甘蔗生长触发，
///   蘑菇扩散或其他非作物生长路径。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockGrowEvent {
    /// 正在发生生长的世界。
    pub world: Arc<World>,

    /// 生长前的原始方块。
    pub old_block: &'static Block,

    /// 原始方块状态 ID。
    pub old_state_id: BlockStateId,

    /// 生长所指向的新方块。
    pub new_block: &'static Block,

    /// 要应用的新方块状态 id。
    pub new_state_id: BlockStateId,

    /// 正在生长的方块的位置。
    pub block_pos: BlockPos,
}

impl BlockGrowEvent {
    /// 创建新的 `BlockGrowEvent`。
    ///
    /// # Arguments
    /// - `world`：发生生长的世界。
    /// - `old_block`：生长前的原始方块。
    /// - `old_state_id`：原始方块状态 ID。
    /// - `new_block`：生长所指向的新方块。
    /// - `new_state_id`：若未取消则将应用的新方块状态 ID。
    /// - `block_pos`：发生生长的方块位置。
    ///
    /// # Returns
    /// 一个新的 `BlockGrowEvent`。
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        old_block: &'static Block,
        old_state_id: BlockStateId,
        new_block: &'static Block,
        new_state_id: BlockStateId,
        block_pos: BlockPos,
    ) -> Self {
        Self {
            world,
            old_block,
            old_state_id,
            new_block,
            new_state_id,
            block_pos,
            cancelled: false,
        }
    }
}

impl BlockEvent for BlockGrowEvent {
    fn get_block(&self) -> &Block {
        self.old_block
    }
}
