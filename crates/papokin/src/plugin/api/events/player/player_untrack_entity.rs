use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 实体停止被玩家追踪时发生的事件（该实体随之对玩家消失）
/// 该玩家客户端侧的实体会被销毁）。
///
/// 纯通知；若要否决可见性变更，请使用
/// 应改用 [`super::player_hide_entity::PlayerHideEntityEvent`]。
#[derive(Event, Clone)]
pub struct PlayerUntrackEntityEvent {
    /// 停止追踪实体的玩家。
    pub player: Arc<Player>,

    /// 不再被追踪的实体的实体 ID。
    pub entity_id: i32,
}

impl PlayerUntrackEntityEvent {
    /// 创建 `PlayerUntrackEntityEvent` 的新实例。
    pub const fn new(player: Arc<Player>, entity_id: i32) -> Self {
        Self { player, entity_id }
    }
}

impl PlayerEvent for PlayerUntrackEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
