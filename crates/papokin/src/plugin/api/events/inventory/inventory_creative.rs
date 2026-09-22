use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 创造模式玩家设置物品栏槽位时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryCreativeEvent {
    /// 执行创造模式操作的玩家。
    pub player: Arc<Player>,

    /// 目标槽位索引。
    pub slot: i16,

    /// 设置到槽位中的物品 ID。
    pub item_id: String,

    /// 设置的物品数量。
    pub item_count: u8,
}

impl InventoryCreativeEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, slot: i16, item_id: String, item_count: u8) -> Self {
        Self {
            player,
            slot,
            item_id,
            item_count,
            cancelled: false,
        }
    }
}
