use crate::entity::player::Player;
use papokin_macros::Event;
use std::sync::Arc;

/// 物品栏结果槽位物品被准备时发生的通用事件。
#[derive(Event, Clone)]
pub struct PrepareInventoryResultEvent {
    /// 与物品栏交互的玩家。
    pub player: Arc<Player>,

    /// 结果物品 ID（如有）。
    pub result_item: Option<String>,
}

impl PrepareInventoryResultEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, result_item: Option<String>) -> Self {
        Self {
            player,
            result_item,
        }
    }
}
