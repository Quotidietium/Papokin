use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 实体开始被玩家追踪（生成/发送）时发生的事件
/// 一名玩家。
///
/// 纯通知；若要否决可见性，请使用
/// 应改用 [`super::player_show_entity::PlayerShowEntityEvent`]。
#[derive(Event, Clone)]
pub struct PlayerTrackEntityEvent {
    /// 开始追踪实体的玩家。
    pub player: Arc<Player>,

    /// 正在被追踪的实体的实体 ID。
    pub entity_id: i32,
}

impl PlayerTrackEntityEvent {
    /// 创建 `PlayerTrackEntityEvent` 的新实例。
    pub const fn new(player: Arc<Player>, entity_id: i32) -> Self {
        Self { player, entity_id }
    }
}

impl PlayerEvent for PlayerTrackEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
