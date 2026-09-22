use std::sync::Arc;

use papokin_data::entity::EntityType;
use papokin_macros::{Event, cancellable};

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家投掷的鸡蛋击中物体时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerEggThrowEvent {
    /// 扔出鸡蛋的玩家。
    pub player: Arc<Player>,

    /// 鸡蛋实体的 UUID。
    pub egg_uuid: uuid::Uuid,

    /// 蛋是否应当孵化。
    pub hatching: bool,

    /// 应孵化的实体数量。
    pub num_hatches: u8,

    /// 应当孵化的实体类型。
    pub hatching_type: &'static EntityType,
}

impl PlayerEggThrowEvent {
    /// 创建 `PlayerEggThrowEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        egg_uuid: uuid::Uuid,
        hatching: bool,
        num_hatches: u8,
        hatching_type: &'static EntityType,
    ) -> Self {
        Self {
            player,
            egg_uuid,
            hatching,
            num_hatches,
            hatching_type,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerEggThrowEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
