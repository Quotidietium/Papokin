use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 物品冷却组被应用冷却（对
/// 针对玩家。
///
/// 与 [`super::player_item_cooldown::PlayerItemCooldownEvent`] 一同（在其之后）触发
/// 每当应用冷却时。取消可阻止该冷却被
/// 已应用。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemGroupCooldownEvent {
    /// 接收冷却的玩家。
    pub player: Arc<Player>,

    /// 冷却组的标识符。
    pub cooldown_group: String,

    /// 冷却时长（以刻为单位）。
    pub cooldown: i32,
}

impl PlayerItemGroupCooldownEvent {
    /// 创建 `PlayerItemGroupCooldownEvent` 的新实例。
    pub fn new(player: Arc<Player>, cooldown_group: impl Into<String>, cooldown: i32) -> Self {
        Self {
            player,
            cooldown_group: cooldown_group.into(),
            cooldown,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemGroupCooldownEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
