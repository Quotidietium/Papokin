use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家切换疾跑状态时发生的事件。
///
/// 此事件包含玩家及其新的疾跑状态。
/// 可取消该事件以阻止疾跑状态变更。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerToggleSprintEvent {
    /// 切换了疾跑状态的玩家。
    pub player: Arc<Player>,

    /// 玩家的新疾跑状态。
    pub is_sprinting: bool,
}

impl PlayerToggleSprintEvent {
    /// 创建 `PlayerToggleSprintEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：切换了疾跑状态的玩家的引用。
    /// - `is_sprinting`：新的疾跑状态。
    ///
    /// # Returns
    /// 一个新的 `PlayerToggleSprintEvent` 实例。
    pub const fn new(player: Arc<Player>, is_sprinting: bool) -> Self {
        Self {
            player,
            is_sprinting,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerToggleSprintEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
