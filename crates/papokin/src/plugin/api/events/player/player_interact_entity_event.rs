use std::sync::Arc;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_protocol::java::server::play::ActionType;
use papokin_util::math::vector3::Vector3;

use super::PlayerEvent;

/// 玩家与实体交互时触发的事件。
///
/// 此事件会为所有实体交互类型触发：交互（右键点击）、
/// 攻击（左键单击），以及定点交互（在特定位置右键单击）。
/// 可以取消它以阻止默认交互行为。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInteractEntityEvent {
    /// 执行交互的玩家。
    pub player: Arc<Player>,

    /// 被交互的实体。
    pub target: Arc<dyn EntityBase>,

    /// 交互类型（Interact、Attack 或 `InteractAt`）。
    pub action: ActionType,

    /// 实体上被点击的位置（仅用于 `InteractAt`）。
    pub target_position: Option<Vector3<f64>>,

    /// 交互期间玩家是否在潜行。
    pub sneaking: bool,
}

impl PlayerInteractEntityEvent {
    pub fn new(
        player: &Arc<Player>,
        target: Arc<dyn EntityBase>,
        action: ActionType,
        target_position: Option<Vector3<f64>>,
        sneaking: bool,
    ) -> Self {
        Self {
            player: Arc::clone(player),
            target,
            action,
            target_position,
            sneaking,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerInteractEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
