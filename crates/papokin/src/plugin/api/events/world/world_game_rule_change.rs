use crate::world::World;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 世界中的游戏规则值变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldGameRuleChangeEvent {
    /// 游戏规则发生变化的世界。
    pub world: Arc<World>,

    /// 游戏规则的名称。
    pub rule: String,

    /// 游戏规则的新值。
    pub value: String,
}

impl WorldGameRuleChangeEvent {
    #[must_use]
    pub const fn new(world: Arc<World>, rule: String, value: String) -> Self {
        Self {
            world,
            rule,
            value,
            cancelled: false,
        }
    }
}
