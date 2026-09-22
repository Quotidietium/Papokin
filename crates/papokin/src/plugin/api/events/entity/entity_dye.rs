use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 可用于给实体染色的 16 种颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyeColor {
    White,
    Orange,
    Magenta,
    LightBlue,
    Yellow,
    Lime,
    Pink,
    Gray,
    LightGray,
    Cyan,
    Purple,
    Blue,
    Brown,
    Green,
    Red,
    Black,
}

/// 实体被染色时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDyeEvent {
    /// 被染色实体的 ID。
    pub entity_id: i32,

    /// 已应用的颜色。
    pub color: DyeColor,

    /// 给实体染色的玩家（如果有的话）。
    pub player: Option<Arc<Player>>,
}

impl EntityDyeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, color: DyeColor, player: Option<Arc<Player>>) -> Self {
        Self {
            entity_id,
            color,
            player,
            cancelled: false,
        }
    }
}
