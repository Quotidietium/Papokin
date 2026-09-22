use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 玩家上床时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerBedEnterEvent {
    /// 上床的玩家。
    pub player: Arc<Player>,

    /// 床的位置。
    pub bed_pos: BlockPos,
}

impl PlayerBedEnterEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, bed_pos: BlockPos) -> Self {
        Self {
            player,
            bed_pos,
            cancelled: false,
        }
    }
}

/// 玩家离开床时发生的事件。
#[derive(Event, Clone)]
pub struct PlayerBedLeaveEvent {
    /// 下床的玩家。
    pub player: Arc<Player>,

    /// 床的位置。
    pub bed_pos: BlockPos,
}

impl PlayerBedLeaveEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, bed_pos: BlockPos) -> Self {
        Self { player, bed_pos }
    }
}
