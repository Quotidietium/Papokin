use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家用命名牌为实体命名时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerNameEntityEvent {
    /// 给实体命名的玩家。
    pub player: Arc<Player>,

    /// 被命名实体的 ID。
    pub entity_id: i32,

    /// 已应用的自定义名称。
    pub name: TextComponent,
}

impl PlayerEvent for PlayerNameEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
