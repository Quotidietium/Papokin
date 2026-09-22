use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家更改手持快捷栏槽位时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemHeldEvent {
    /// 更改手持栏位的玩家。
    pub player: Arc<Player>,

    /// 之前的快捷栏槽位。
    pub previous_slot: u8,

    /// 新的快捷栏槽位。
    pub new_slot: u8,
}

impl PlayerItemHeldEvent {
    /// 创建 `PlayerItemHeldEvent` 的新实例。
    pub const fn new(player: Arc<Player>, previous_slot: u8, new_slot: u8) -> Self {
        Self {
            player,
            previous_slot,
            new_slot,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemHeldEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
