use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家与村民或流浪商人交易时触发的事件。
/// 交易者。
///
/// 取消即否决交易。当交易对象……时，`merchant_id` 为 `-1`
/// 界面背后没有存活的商人实体（WIT 字段只是一个普通
/// `s32`，因此使用哨兵值来代替 Option）。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerTradeEvent {
    /// 进行交易的玩家。
    pub player: Arc<Player>,

    /// 商人实体的 id（没有商人实体时为 `-1`）。
    pub merchant_id: i32,

    /// 该交易的原料物品。
    pub ingredients: Vec<ItemStack>,

    /// 交易得到的结果物品。
    pub result: ItemStack,

    /// 村民从此交易中获得的经验。
    pub villager_experience: i32,
}

impl PlayerTradeEvent {
    /// 创建 `PlayerTradeEvent` 的新实例。
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        merchant_id: i32,
        ingredients: Vec<ItemStack>,
        result: ItemStack,
        villager_experience: i32,
    ) -> Self {
        Self {
            player,
            merchant_id,
            ingredients,
            result,
            villager_experience,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerTradeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
