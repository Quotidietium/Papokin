use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家使用传送门时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPortalEvent {
    /// 使用传送门的玩家。
    pub player: Arc<Player>,

    /// 玩家进入的传送门位置。
    pub from_pos: BlockPos,

    /// 目标位置（如果已知）。
    pub to_pos: Option<BlockPos>,
}

impl PlayerEvent for PlayerPortalEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
