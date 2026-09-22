use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家与实体上特定位置交互时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInteractAtEntityEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 目标实体 ID。
    pub entity_id: i32,
    /// 点击位置的 X 坐标。
    pub clicked_x: f64,
    /// 点击位置的 Y 坐标。
    pub clicked_y: f64,
    /// 点击位置的 Z 坐标。
    pub clicked_z: f64,
    /// 所用的手（0 = 主手，1 = 副手）。
    pub hand: u8,
}

impl PlayerInteractAtEntityEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        entity_id: i32,
        clicked_x: f64,
        clicked_y: f64,
        clicked_z: f64,
        hand: u8,
    ) -> Self {
        Self {
            player,
            entity_id,
            clicked_x,
            clicked_y,
            clicked_z,
            hand,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerInteractAtEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
