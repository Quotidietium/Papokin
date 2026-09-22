use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家的重生点变化时发生的事件（如床/重生锚）。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSpawnChangeEvent {
    /// 生成点发生变化的玩家。
    pub player: Arc<Player>,

    /// 新的生成点位置（如果已设置）。
    pub new_spawn: Option<BlockPos>,

    /// 此生成点是否为强制指定。
    pub forced: bool,
}

impl PlayerEvent for PlayerSpawnChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
