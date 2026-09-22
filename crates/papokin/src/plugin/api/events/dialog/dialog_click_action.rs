use bytes::Bytes;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::super::player::PlayerEvent;

/// 玩家点击自定义对话框按钮时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct DialogClickActionEvent {
    /// 点击按钮的玩家。
    pub player: Arc<Player>,
    /// 此操作的唯一标识符。
    pub id: String,
    /// 与该操作关联的可选二进制数据。
    pub payload: Option<Bytes>,
}

impl DialogClickActionEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, id: String, payload: Option<Bytes>) -> Self {
        Self {
            player,
            id,
            payload,
            cancelled: false,
        }
    }
}

impl PlayerEvent for DialogClickActionEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
