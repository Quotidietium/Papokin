use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家更改客户端语言/区域设置时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLocaleChangeEvent {
    /// 相关的玩家。
    pub player: Arc<Player>,

    /// 新的语言代码（例如 "`en_us`"）。
    pub new_locale: String,
}

impl PlayerEvent for PlayerLocaleChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
