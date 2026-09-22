use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// [`PlayerBedFailEnterEvent::fail_reason`] 的已知取值。
///
/// 对应 Paper 的 `PlayerBedFailEnterEvent.FailReason` 名称。
pub mod bed_fail_reasons {
    /// 在此维度中床完全无法用于睡觉。
    pub const NOT_POSSIBLE_HERE: &str = "not_possible_here";
    /// 当前的时间或天气不允许睡觉。
    pub const NOT_POSSIBLE_NOW: &str = "not_possible_now";
    /// 玩家距离床太远。
    pub const TOO_FAR_AWAY: &str = "too_far_away";
    /// 床被阻挡。
    pub const OBSTRUCTED: &str = "obstructed";
    /// 床已被占用。
    pub const OCCUPIED: &str = "occupied";
    /// 附近有怪物。
    pub const NOT_SAFE: &str = "not_safe";
}

/// 玩家尝试进入床失败时发生的事件。
///
/// 取消该事件会抑制失败：不发送消息，且
/// 玩家获准继续进入床。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerBedFailEnterEvent {
    /// 未能上床的玩家。
    pub player: Arc<Player>,

    /// 床的位置（床头方块）。
    pub bed_pos: BlockPos,

    /// 上床失败的原因（见 [`bed_fail_reasons`]）。
    pub fail_reason: String,
}

impl PlayerBedFailEnterEvent {
    /// 创建 `PlayerBedFailEnterEvent` 的新实例。
    pub fn new(player: Arc<Player>, bed_pos: BlockPos, fail_reason: impl Into<String>) -> Self {
        Self {
            player,
            bed_pos,
            fail_reason: fail_reason.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerBedFailEnterEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
