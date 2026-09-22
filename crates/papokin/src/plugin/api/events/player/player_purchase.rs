use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家与商人完成交易时触发的事件。
///
/// 取消即否决交易。当交易对象……时，`merchant_id` 为 `None`
/// 界面背后没有存活的商人实体（例如自定义商人
/// 菜单）；否则为商人实体的 id。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPurchaseEvent {
    /// 进行购买的玩家。
    pub player: Arc<Player>,

    /// 商人实体的 id，如果有的话。
    pub merchant_id: Option<i32>,

    /// 该交易的原料物品。
    pub ingredients: Vec<ItemStack>,

    /// 交易得到的结果物品。
    pub result: ItemStack,
}

impl PlayerPurchaseEvent {
    /// 创建 `PlayerPurchaseEvent` 的新实例。
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        merchant_id: Option<i32>,
        ingredients: Vec<ItemStack>,
        result: ItemStack,
    ) -> Self {
        Self {
            player,
            merchant_id,
            ingredients,
            result,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPurchaseEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
