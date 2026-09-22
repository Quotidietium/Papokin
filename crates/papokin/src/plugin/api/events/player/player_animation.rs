use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家执行的动画类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAnimationType {
    ArmSwingMain,
    ArmSwingOff,
    LeaveBed,
}

/// 玩家播放动作动画时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerAnimationEvent {
    /// 执行动画的玩家。
    pub player: Arc<Player>,

    /// 动画类型。
    pub animation_type: PlayerAnimationType,
}

impl PlayerAnimationEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, animation_type: PlayerAnimationType) -> Self {
        Self {
            player,
            animation_type,
            cancelled: false,
        }
    }
}
