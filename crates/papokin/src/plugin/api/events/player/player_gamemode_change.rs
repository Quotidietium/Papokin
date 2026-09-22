use papokin_macros::{Event, cancellable};
use papokin_util::GameMode;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家更改游戏模式时发生的事件。
///
/// 此事件包含以下信息
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerGamemodeChangeEvent {
    /// 游戏模式正在变化的玩家。
    pub player: Arc<Player>,

    /// 玩家之前的游戏模式。
    pub previous_gamemode: GameMode,

    /// 玩家的新游戏模式。
    pub new_gamemode: GameMode,
}

impl PlayerGamemodeChangeEvent {
    /// 创建 `PlayerGamemodeChangeEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在切换游戏模式的玩家的引用。
    /// - `previous_gamemode`：玩家先前的游戏模式。
    /// - `new_gamemode`：玩家的新游戏模式。
    ///
    /// # Returns
    /// 一个新的 `PlayerGamemodeChangeEvent` 实例。
    pub const fn new(
        player: Arc<Player>,
        previous_gamemode: GameMode,
        new_gamemode: GameMode,
    ) -> Self {
        Self {
            player,
            previous_gamemode,
            new_gamemode,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerGamemodeChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
