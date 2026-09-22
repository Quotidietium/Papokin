use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家切换飞行状态时发生的事件。
///
/// 此事件包含玩家及其新的飞行状态。
/// 可取消该事件以阻止飞行状态变更。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerToggleFlightEvent {
    /// 切换了飞行状态的玩家。
    pub player: Arc<Player>,

    /// 玩家的新飞行状态。
    pub is_flying: bool,
}

impl PlayerToggleFlightEvent {
    /// 创建 `PlayerToggleFlightEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：切换了飞行状态的玩家的引用。
    /// - `is_flying`：新的飞行状态。
    ///
    /// # Returns
    /// 一个新的 `PlayerToggleFlightEvent` 实例。
    pub const fn new(player: Arc<Player>, is_flying: bool) -> Self {
        Self {
            player,
            is_flying,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerToggleFlightEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
