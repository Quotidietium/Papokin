use papokin_macros::{Event, cancellable};

/// 实体拾取物品堆时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPickupItemEvent {
    /// 拾取实体的 ID。
    pub entity_id: i32,

    /// 物品的注册表名称。
    pub item_name: String,

    /// 拾取的物品数量。
    pub count: u8,
}

impl EntityPickupItemEvent {
    #[must_use]
    pub const fn new(entity_id: i32, item_name: String, count: u8) -> Self {
        Self {
            entity_id,
            item_name,
            count,
            cancelled: false,
        }
    }
}
