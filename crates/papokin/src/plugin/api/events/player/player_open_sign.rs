use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 为玩家打开告示牌编辑器时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerOpenSignEvent {
    /// 打开告示牌的玩家。
    pub player: Arc<Player>,

    /// 告示牌方块的位置。
    pub block_pos: BlockPos,

    /// 是否正在编辑正面。
    pub is_front: bool,
}

impl PlayerEvent for PlayerOpenSignEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
