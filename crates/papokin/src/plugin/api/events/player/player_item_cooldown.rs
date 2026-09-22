use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 某物品类型被应用冷却（对
/// 玩家。
///
/// 在此实现中，物品冷却以冷却组为键；
/// 当冷却键指明具体物品类型时触发（默认
/// 当物品未设置显式冷却组时）。取消可阻止
/// 冷却时间被应用。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemCooldownEvent {
    /// 接收冷却的玩家。
    pub player: Arc<Player>,

    /// 物品类型的标识符（例如 `minecraft:ender_pearl`）。
    pub item_type: String,

    /// 冷却时长（以刻为单位）。
    pub cooldown: i32,
}

impl PlayerItemCooldownEvent {
    /// 创建 `PlayerItemCooldownEvent` 的新实例。
    pub fn new(player: Arc<Player>, item_type: impl Into<String>, cooldown: i32) -> Self {
        Self {
            player,
            item_type: item_type.into(),
            cooldown,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemCooldownEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
