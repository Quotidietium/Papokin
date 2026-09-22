use papokin_macros::{Event, cancellable};

/// 村民补充交易时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VillagerReplenishTradeEvent {
    /// 村民实体的 ID。
    pub entity_id: i32,

    /// 该交易补货的次数。
    pub restock_quantity: i32,
}
