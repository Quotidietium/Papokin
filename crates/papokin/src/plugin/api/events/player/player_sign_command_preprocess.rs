use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 告示牌上的命令在
/// 执行。
///
/// 取消会阻止该命令被执行；`command` 可以被
/// 可被处理器修改，以便在命令运行前重写命令。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSignCommandPreprocessEvent {
    /// 执行告示牌命令的玩家。
    pub player: Arc<Player>,

    /// 告示牌方块的位置。
    pub block_pos: BlockPos,

    /// 将要执行的命令（可修改）。
    pub command: String,
}

impl PlayerSignCommandPreprocessEvent {
    /// 创建 `PlayerSignCommandPreprocessEvent` 的新实例。
    pub fn new(player: Arc<Player>, block_pos: BlockPos, command: impl Into<String>) -> Self {
        Self {
            player,
            block_pos,
            command: command.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerSignCommandPreprocessEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
