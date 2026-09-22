use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// 信标被停用（其等级降为 0）时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BeaconDeactivatedEvent {
    /// 停用信标的归属玩家（如果有的话）
    /// (观察到状态转换时范围内的最近玩家)。
    pub player: Option<Arc<Player>>,

    /// 信标方块的位置。
    pub block_pos: BlockPos,
}

impl BeaconDeactivatedEvent {
    #[must_use]
    pub const fn new(player: Option<Arc<Player>>, block_pos: BlockPos) -> Self {
        Self {
            player,
            block_pos,
            cancelled: false,
        }
    }
}
