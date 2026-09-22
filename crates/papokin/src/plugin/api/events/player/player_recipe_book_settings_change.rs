use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家更改配方书设置时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerRecipeBookSettingsChangeEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 配方书类型（合成、熔炉等）。
    pub book_type: String,
    /// 书本是否打开。
    pub is_open: bool,
    /// 过滤器是否处于激活状态。
    pub is_filtering: bool,
}

impl PlayerRecipeBookSettingsChangeEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        book_type: String,
        is_open: bool,
        is_filtering: bool,
    ) -> Self {
        Self {
            player,
            book_type,
            is_open,
            is_filtering,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerRecipeBookSettingsChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
