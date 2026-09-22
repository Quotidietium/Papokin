use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家拾取经验球时发生的事件。
///
/// 取消会完全阻止拾取。`amount` 可以被
/// 处理程序，该值是经验修补与经验获取将要消耗的量。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickupExperienceEvent {
    /// 拾取经验的玩家。
    pub player: Arc<Player>,

    /// 经验球的实体 ID。
    pub orb_id: i32,

    /// 拾取的经验数量。
    pub amount: i32,
}

impl PlayerPickupExperienceEvent {
    /// 创建 `PlayerPickupExperienceEvent` 的新实例。
    pub const fn new(player: Arc<Player>, orb_id: i32, amount: i32) -> Self {
        Self {
            player,
            orb_id,
            amount,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickupExperienceEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
