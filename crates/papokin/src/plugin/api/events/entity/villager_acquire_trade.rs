use papokin_macros::{Event, cancellable};

/// 村民获得新交易时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VillagerAcquireTradeEvent {
    /// 村民实体的 ID。
    pub entity_id: i32,

    /// 交易配方的索引。
    pub recipe_index: i32,
}
