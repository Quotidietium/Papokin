use std::sync::Arc;

use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_protocol::java::server::play::ActionType;

use super::PlayerEvent;

/// 玩家与一个在世界中未找到的实体交互时触发的事件。
///
/// 当目标实体已被移除或服务器因其他原因不知晓该实体时，就会发生这种情况。
/// 可以取消它以阻止默认行为（例如踢出玩家）。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInteractUnknownEntityEvent {
    /// 执行交互的玩家。
    pub player: Arc<Player>,

    /// 被作为目标的实体 ID。
    pub entity_id: i32,

    /// 交互类型（Interact、Attack 或 `InteractAt`）。
    pub action: ActionType,
}

impl PlayerInteractUnknownEntityEvent {
    pub fn new(player: &Arc<Player>, entity_id: i32, action: ActionType) -> Self {
        Self {
            player: Arc::clone(player),
            entity_id,
            action,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerInteractUnknownEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
