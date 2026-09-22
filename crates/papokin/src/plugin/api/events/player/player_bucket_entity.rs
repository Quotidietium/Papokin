use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家用桶捕捉实体时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerBucketEntityEvent {
    /// 使用桶的玩家。
    pub player: Arc<Player>,

    /// 被捕捉实体的 ID。
    pub entity_id: i32,

    /// 生成的桶物品名称。
    pub bucket_item: String,
}

impl PlayerEvent for PlayerBucketEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
