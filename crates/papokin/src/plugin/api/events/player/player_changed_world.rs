use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;
use crate::world::World;

/// 玩家切换世界之后触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerChangedWorldEvent {
    /// 切换了世界的玩家。
    pub player: Arc<Player>,

    /// 玩家来自的世界。
    pub from_world: Arc<World>,

    /// 玩家到达的世界。
    pub to_world: Arc<World>,
}

impl PlayerEvent for PlayerChangedWorldEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
