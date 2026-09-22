use papokin_macros::{Event, cancellable};
use papokin_util::Hand;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 表示钓鱼事件可能状态的枚举。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayerFishState {
    /// 抛出钓竿的玩家。
    Fishing,

    /// 鱼钩钓到了鱼。
    CaughtFish,

    /// 鱼钩钩住了一个实体。
    CaughtEntity,

    /// 鱼钩落在了地上。
    InGround,

    /// 钓鱼尝试失败。
    FailedAttempt,

    /// 收回鱼钩的玩家。
    ReelIn,

    /// 一条鱼咬钩了。
    Bite,
}

/// 玩家钓鱼时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerFishEvent {
    /// 正在钓鱼的玩家。
    pub player: Arc<Player>,

    /// 被捕获实体的 UUID（如有）。
    pub caught_uuid: Option<uuid::Uuid>,

    /// 被捕获实体的类型（注册表键）。
    pub caught_type: String,

    /// 钓鱼钩的 UUID。
    pub hook_uuid: uuid::Uuid,

    /// 钓鱼事件的状态。
    pub state: PlayerFishState,

    /// 用于钓鱼的手。
    pub hand: Hand,

    /// 掉落的经验。
    pub exp_to_drop: i32,
}

impl PlayerFishEvent {
    /// 创建 `PlayerFishEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        caught_uuid: Option<uuid::Uuid>,
        hook_uuid: uuid::Uuid,
        caught_type: String,
        state: PlayerFishState,
        hand: Hand,
        exp_to_drop: i32,
    ) -> Self {
        Self {
            player,
            caught_uuid,
            hook_uuid,
            caught_type,
            state,
            hand,
            exp_to_drop,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerFishEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
