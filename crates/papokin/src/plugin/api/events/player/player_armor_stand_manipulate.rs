use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家操作盔甲架时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerArmorStandManipulateEvent {
    /// 操作盔甲架的玩家。
    pub player: Arc<Player>,

    /// 盔甲架实体的 ID。
    pub armor_stand_id: i32,

    /// 盔甲架槽位。
    pub slot: u8,
}

impl PlayerEvent for PlayerArmorStandManipulateEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
