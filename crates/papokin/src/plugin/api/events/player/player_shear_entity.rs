use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家剪实体时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerShearEntityEvent {
    /// 执行剪毛的玩家。
    pub player: Arc<Player>,

    /// 被剪毛的实体 ID。
    pub entity_id: i32,

    /// 所用的手（0 = 主手，1 = 副手）。
    pub hand: u8,
}

impl PlayerEvent for PlayerShearEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
