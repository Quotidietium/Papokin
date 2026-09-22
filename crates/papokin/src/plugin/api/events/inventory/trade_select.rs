use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家与商人选择交易配方时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct TradeSelectEvent {
    /// 选择交易的玩家。
    pub player: Arc<Player>,

    /// 所选交易槽位的索引。
    pub slot_index: u8,
}

impl TradeSelectEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, slot_index: u8) -> Self {
        Self {
            player,
            slot_index,
            cancelled: false,
        }
    }
}
