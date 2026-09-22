use papokin_macros::{Event, cancellable};
use papokin_protocol::java::client::dialog::Dialog;
use std::sync::Arc;

use crate::entity::player::Player;

use super::super::player::PlayerEvent;

/// 向玩家展示对话框时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct DialogShowEvent {
    /// 接收对话框的玩家。
    pub player: Arc<Player>,
    /// 正在显示的对话框。
    pub dialog: Dialog,
}

impl DialogShowEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, dialog: Dialog) -> Self {
        Self {
            player,
            dialog,
            cancelled: false,
        }
    }
}

impl PlayerEvent for DialogShowEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
