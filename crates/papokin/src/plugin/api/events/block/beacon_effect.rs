use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// 信标向玩家施加状态效果时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BeaconEffectEvent {
    /// 接收效果的玩家。
    pub player: Arc<Player>,

    /// 正在施加的效果的标识符（例如 `minecraft:speed`）。
    pub effect: String,

    /// 该效果是否为信标的主效果。
    pub primary: bool,

    /// 信标方块的位置。
    pub block_pos: BlockPos,
}

impl BeaconEffectEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        effect: String,
        primary: bool,
        block_pos: BlockPos,
    ) -> Self {
        Self {
            player,
            effect,
            primary,
            block_pos,
            cancelled: false,
        }
    }
}
