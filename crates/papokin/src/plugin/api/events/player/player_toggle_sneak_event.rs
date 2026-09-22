use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家切换潜行状态时发生的事件。
///
/// 此事件包含玩家及其新的潜行状态。
/// 可取消该事件以阻止潜行状态变更。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerToggleSneakEvent {
    /// 切换了潜行状态的玩家。
    pub player: Arc<Player>,

    /// 玩家的新潜行状态。
    pub is_sneaking: bool,
}

impl PlayerToggleSneakEvent {
    /// 创建 `PlayerToggleSneakEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：切换了潜行状态的玩家的引用。
    /// - `is_sneaking`：新的潜行状态。
    ///
    /// # Returns
    /// 一个新的 `PlayerToggleSneakEvent` 实例。
    pub const fn new(player: Arc<Player>, is_sneaking: bool) -> Self {
        Self {
            player,
            is_sneaking,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerToggleSneakEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
