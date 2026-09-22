use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家编辑或署名书本时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerEditBookEvent {
    /// 编辑书的玩家。
    pub player: Arc<Player>,

    /// 书本所在的物品栏槽位。
    pub slot: u32,

    /// 书的页面。
    pub pages: Vec<String>,

    /// 书的标题（如果是署名）。
    pub title: Option<String>,

    /// 书本是否正在签名。
    pub signing: bool,
}

impl PlayerEvent for PlayerEditBookEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
