use papokin_macros::{Event, cancellable};

/// 物品实体因存在时间耗尽而消失时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ItemDespawnEvent {
    /// 物品实体的 ID。
    pub entity_id: i32,
}

impl ItemDespawnEvent {
    #[must_use]
    pub const fn new(entity_id: i32) -> Self {
        Self {
            entity_id,
            cancelled: false,
        }
    }
}
