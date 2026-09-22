use papokin_macros::{Event, cancellable};

/// 蝙蝠切换睡眠状态时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BatToggleSleepEvent {
    /// 蝙蝠实体 ID。
    pub entity_id: i32,
    /// 蝙蝠现在是否已苏醒。
    pub is_awake: bool,
}

impl BatToggleSleepEvent {
    #[must_use]
    pub const fn new(entity_id: i32, is_awake: bool) -> Self {
        Self {
            entity_id,
            is_awake,
            cancelled: false,
        }
    }
}
