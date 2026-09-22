use papokin_data::BlockDirection;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::{EntityBase, player::Player};

/// 悬挂实体被放置时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct HangingPlaceEvent {
    /// 正在放置的悬挂实体。
    pub entity: Arc<dyn EntityBase>,
    /// 放置实体的玩家（如果是由玩家放置的）。
    pub player: Option<Arc<Player>>,
    /// 放置实体所参照的方块的位置。
    pub block_pos: BlockPos,
    /// 实体所放置的方块面。
    pub block_face: BlockDirection,
}

impl HangingPlaceEvent {
    #[must_use]
    pub const fn new(
        entity: Arc<dyn EntityBase>,
        player: Option<Arc<Player>>,
        block_pos: BlockPos,
        block_face: BlockDirection,
    ) -> Self {
        Self {
            entity,
            player,
            block_pos,
            block_face,
            cancelled: false,
        }
    }
}
