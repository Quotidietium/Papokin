use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 物品在锻造台中被合成/升级时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SmithItemEvent {
    /// 从锻造输出栏取走物品的玩家。
    pub player: Arc<Player>,

    /// 用于锻造的配方 ID。
    pub recipe_id: String,
}

impl SmithItemEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, recipe_id: String) -> Self {
        Self {
            player,
            recipe_id,
            cancelled: false,
        }
    }
}
