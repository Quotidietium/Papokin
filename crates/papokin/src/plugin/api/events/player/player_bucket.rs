use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 玩家倒空桶时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerBucketEmptyEvent {
    /// 倒空桶的玩家。
    pub player: Arc<Player>,

    /// 桶被倒空的目标方块位置。
    pub block_pos: BlockPos,

    /// 所使用的桶物品。
    pub bucket: String,
}

impl PlayerBucketEmptyEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, bucket: String) -> Self {
        Self {
            player,
            block_pos,
            bucket,
            cancelled: false,
        }
    }
}

/// 玩家装满桶时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerBucketFillEvent {
    /// 装满桶的玩家。
    pub player: Arc<Player>,

    /// 作为填充目标的目标方块位置。
    pub block_pos: BlockPos,

    /// 所使用的桶物品。
    pub bucket: String,
}

impl PlayerBucketFillEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, bucket: String) -> Self {
        Self {
            player,
            block_pos,
            bucket,
            cancelled: false,
        }
    }
}
