use papokin_macros::{Event, cancellable};

/// 苦力怕被点燃时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CreeperIgniteEvent {
    /// 正被点燃的苦力怕 ID。
    pub entity_id: i32,

    /// 点燃苦力怕的实体 ID（如有）。
    pub igniter_id: Option<i32>,
}

impl CreeperIgniteEvent {
    #[must_use]
    pub const fn new(entity_id: i32, igniter_id: Option<i32>) -> Self {
        Self {
            entity_id,
            igniter_id,
            cancelled: false,
        }
    }
}
