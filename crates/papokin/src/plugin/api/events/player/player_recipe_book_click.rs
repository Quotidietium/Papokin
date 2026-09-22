use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家在配方书中点击配方时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerRecipeBookClickEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 配方标识符。
    pub recipe_id: String,
    /// 是否按住了 Shift（合成最大数量）。
    pub make_all: bool,
}

impl PlayerRecipeBookClickEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, recipe_id: String, make_all: bool) -> Self {
        Self {
            player,
            recipe_id,
            make_all,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerRecipeBookClickEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
