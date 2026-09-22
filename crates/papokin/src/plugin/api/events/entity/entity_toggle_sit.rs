use papokin_macros::{Event, cancellable};

/// 实体切换坐下状态时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityToggleSitEvent {
    /// 切换坐下状态的实体 ID。
    pub entity_id: i32,

    /// 实体现在是否处于坐姿。
    pub sitting: bool,
}

impl EntityToggleSitEvent {
    #[must_use]
    pub const fn new(entity_id: i32, sitting: bool) -> Self {
        Self {
            entity_id,
            sitting,
            cancelled: false,
        }
    }
}
