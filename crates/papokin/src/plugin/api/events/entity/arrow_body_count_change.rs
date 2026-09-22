use papokin_macros::{Event, cancellable};

/// 实体身上插着的箭数量变化时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ArrowBodyCountChangeEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 原来的箭数量。
    pub old_amount: u32,
    /// 新的箭数量。
    pub new_amount: u32,
}

impl ArrowBodyCountChangeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, old_amount: u32, new_amount: u32) -> Self {
        Self {
            entity_id,
            old_amount,
            new_amount,
            cancelled: false,
        }
    }
}
