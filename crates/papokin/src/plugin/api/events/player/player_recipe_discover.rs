use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家发现配方时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerRecipeDiscoverEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 配方 ID。
    pub recipe_id: String,
}

impl PlayerRecipeDiscoverEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, recipe_id: String) -> Self {
        Self {
            player,
            recipe_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerRecipeDiscoverEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
