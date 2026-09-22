use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// 信标被激活（其等级从 0 变为 n）时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BeaconActivatedEvent {
    /// 激活信标的归属玩家（范围内最近的玩家）。
    pub player: Arc<Player>,

    /// 信标方块的位置。
    pub block_pos: BlockPos,
}

impl BeaconActivatedEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos) -> Self {
        Self {
            player,
            block_pos,
            cancelled: false,
        }
    }
}
