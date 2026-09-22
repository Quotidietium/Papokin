use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 对物品展示框执行的操作。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemFrameAction {
    /// 物品被放入展示框。
    Place,

    /// 物品从展示框中被移除。
    Remove,

    /// 展示框中的物品被旋转。
    Rotate,
}

/// 玩家在物品展示框中放置、移除或旋转物品时发生的事件。
/// 物品展示框。
///
/// 取消即否决该更改。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerItemFrameChangeEvent {
    /// 与物品展示框交互的玩家。
    pub player: Arc<Player>,

    /// 物品展示框的实体 ID。
    pub frame_id: i32,

    /// 此操作涉及的物品（被放置/移除的物品，或
    /// 旋转时当前所显示的）。
    pub item: ItemStack,

    /// 对物品展示框执行的操作。
    pub action: ItemFrameAction,
}

impl PlayerItemFrameChangeEvent {
    /// 创建 `PlayerItemFrameChangeEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        frame_id: i32,
        item: ItemStack,
        action: ItemFrameAction,
    ) -> Self {
        Self {
            player,
            frame_id,
            item,
            action,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerItemFrameChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
