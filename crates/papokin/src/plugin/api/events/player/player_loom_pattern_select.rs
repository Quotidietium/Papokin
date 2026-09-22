use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家在织布机中选择图案时发生的事件。
///
/// 取消即否决选择。此事件在 Java 容器按钮……上触发
/// 路径。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLoomPatternSelectEvent {
    /// 选择图案的玩家。
    pub player: Arc<Player>,

    /// 所选旗帜图案的标识符（例如 `minecraft:creeper`）。
    pub pattern: String,
}

impl PlayerLoomPatternSelectEvent {
    /// 创建 `PlayerLoomPatternSelectEvent` 的新实例。
    pub fn new(player: Arc<Player>, pattern: impl Into<String>) -> Self {
        Self {
            player,
            pattern: pattern.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLoomPatternSelectEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
