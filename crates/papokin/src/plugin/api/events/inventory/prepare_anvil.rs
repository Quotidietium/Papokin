use crate::entity::player::Player;
use papokin_macros::Event;
use std::sync::Arc;

/// 物品在铁砧中被处理时发生的事件。
#[derive(Event, Clone)]
pub struct PrepareAnvilEvent {
    /// 使用铁砧的玩家。
    pub player: Arc<Player>,

    /// 铁砧输入文本框中准备的名称。
    pub rename_text: String,

    /// 修复消耗（以经验等级计）。
    pub repair_cost: u32,
}

impl PrepareAnvilEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, rename_text: String, repair_cost: u32) -> Self {
        Self {
            player,
            rename_text,
            repair_cost,
        }
    }
}
