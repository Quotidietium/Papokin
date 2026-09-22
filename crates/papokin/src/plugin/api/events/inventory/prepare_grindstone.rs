use crate::entity::player::Player;
use papokin_macros::Event;
use std::sync::Arc;

/// 物品被放入砂轮时发生的事件。
#[derive(Event, Clone)]
pub struct PrepareGrindstoneEvent {
    /// 使用砂轮的玩家。
    pub player: Arc<Player>,

    /// 在输出槽位中准备好的结果物品 ID。
    pub result_item: Option<String>,
}

impl PrepareGrindstoneEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, result_item: Option<String>) -> Self {
        Self {
            player,
            result_item,
        }
    }
}
