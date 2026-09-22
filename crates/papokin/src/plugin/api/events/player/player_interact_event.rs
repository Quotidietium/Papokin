use std::sync::Arc;

use crate::entity::player::Player;
use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

use super::PlayerEvent;

/// 玩家与方块或空气交互时触发的事件。
///
/// 此事件包含关于玩家、所执行的动作等信息，
/// 玩家手中的物品、交互的方块，以及点击的位置（如有）。
/// 可以取消它以阻止默认交互行为。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInteractEvent {
    /// 执行交互的玩家。
    pub player: Arc<Player>,

    /// 玩家执行的动作类型。
    pub action: InteractAction,

    /// 被点击方块的位置（如果有的话）。
    pub clicked_pos: Option<BlockPos>,

    /// 被交互的方块。
    pub block: &'static Block,
}

impl PlayerInteractEvent {
    /// 创建 `PlayerInteractEvent` 的新实例。
    ///
    /// # Arguments
    ///
    /// - `player`：指向触发该事件的玩家的引用计数指针。
    /// - `action`：所执行的交互类型。
    /// - `block`：被交互的方块。
    /// - `clicked_pos`：被点击方块的可选位置。
    ///
    /// # Returns
    ///
    /// 一个包含指定数据的新 `PlayerInteractEvent` 实例。
    pub fn new(
        player: &Arc<Player>,
        action: InteractAction,
        block: &'static Block,
        clicked_pos: Option<BlockPos>,
    ) -> Self {
        Self {
            player: Arc::clone(player),
            action,
            block,
            clicked_pos,
            cancelled: false,
        }
    }
}

/// 表示玩家可能执行的交互动作的枚举。
#[derive(Clone, PartialEq, Eq)]
pub enum InteractAction {
    /// 左键点击方块
    LeftClickBlock,

    /// 左键点击空气
    LeftClickAir,

    /// 右键点击空气
    RightClickAir,

    /// 右键点击方块
    RightClickBlock,
}

impl InteractAction {
    /// 获取此操作是否由左键点击触发。
    #[must_use]
    #[inline]
    pub fn is_left_click(&self) -> bool {
        Self::LeftClickAir.eq(self) || Self::LeftClickBlock.eq(self)
    }

    /// 获取此操作是否由右键点击触发。
    #[must_use]
    #[inline]
    pub fn is_right_click(&self) -> bool {
        Self::RightClickAir.eq(self) || Self::RightClickBlock.eq(self)
    }
}

impl PlayerEvent for PlayerInteractEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
