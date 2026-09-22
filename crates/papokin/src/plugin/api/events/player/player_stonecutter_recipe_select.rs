use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家在切石机中选择配方时发生的事件。
///
/// 取消即否决选择。`recipe_id` 携带结果物品的
/// 标识符（如 `minecraft:stone_brick_slab`）：切石配方
/// 此引擎没有各自独立的注册表 ID。这是一种
/// Java 协议事件（容器按钮点击）。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStonecutterRecipeSelectEvent {
    /// 选择配方的玩家。
    pub player: Arc<Player>,

    /// 所选配方的标识符。
    pub recipe_id: String,
}

impl PlayerStonecutterRecipeSelectEvent {
    /// 创建 `PlayerStonecutterRecipeSelectEvent` 的新实例。
    pub fn new(player: Arc<Player>, recipe_id: impl Into<String>) -> Self {
        Self {
            player,
            recipe_id: recipe_id.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerStonecutterRecipeSelectEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
