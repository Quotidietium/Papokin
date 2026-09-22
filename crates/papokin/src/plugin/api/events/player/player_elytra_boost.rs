use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家用烟花火箭助推鞘翅飞行时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerElytraBoostEvent {
    /// 正在滑翔的玩家。
    pub player: Arc<Player>,

    /// 烟花实体的 ID。
    pub firework_id: i32,
}

impl PlayerEvent for PlayerElytraBoostEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
