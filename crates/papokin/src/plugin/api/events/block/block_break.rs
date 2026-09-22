use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::BlockEvent;

/// 方块被破坏时触发的事件。
///
/// 此事件包含关于破坏方块的玩家、方块本身等信息，
/// 获得的经验，以及方块是否应当掉落物品。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockBreakEvent {
    /// 正在破坏方块的玩家（如果适用）。
    pub player: Option<Arc<Player>>,

    /// 正在被破坏的方块。
    pub block: &'static Block,

    /// 正被破坏的方块的位置。
    pub block_position: BlockPos,

    /// 破坏该方块获得的经验数量。
    pub exp: u32,

    /// 一个布尔值，表示方块是否应当掉落物品。
    pub drop: bool,
}

impl BlockBreakEvent {
    /// 创建新的 `BlockBreakEvent` 实例。
    ///
    /// # Arguments
    /// - `player`：破坏该方块的玩家的可选引用。
    /// - `block`：正在被破坏的方块。
    /// - `block_position`：正在被破坏的方块的位置。
    /// - `exp`：破坏该方块所获得的经验数量。
    /// - `drop`：指示方块是否应掉落物品的布尔值。
    ///
    /// # Returns
    /// 一个新的 `BlockBreakEvent` 实例。
    #[must_use]
    pub const fn new(
        player: Option<Arc<Player>>,
        block: &'static Block,
        block_position: BlockPos,
        exp: u32,
        drop: bool,
    ) -> Self {
        Self {
            player,
            block,
            block_position,
            exp,
            drop,
            cancelled: false,
        }
    }
}

impl BlockEvent for BlockBreakEvent {
    fn get_block(&self) -> &Block {
        self.block
    }
}
