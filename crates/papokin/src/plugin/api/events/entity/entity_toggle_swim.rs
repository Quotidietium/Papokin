use papokin_macros::{Event, cancellable};

/// 实体开始或停止游泳时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityToggleSwimEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 实体是否正在游泳。
    pub is_swimming: bool,
}

impl EntityToggleSwimEvent {
    #[must_use]
    pub const fn new(entity_id: i32, is_swimming: bool) -> Self {
        Self {
            entity_id,
            is_swimming,
            cancelled: false,
        }
    }
}
