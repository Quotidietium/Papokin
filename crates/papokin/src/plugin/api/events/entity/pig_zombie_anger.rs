use papokin_macros::{Event, cancellable};

/// 僵尸猪灵被激怒时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PigZombieAngerEvent {
    /// 猪僵尸的实体 ID。
    pub entity_id: i32,
    /// 引发仇恨的目标实体 ID。
    pub target_id: Option<i32>,
    /// 新的愤怒等级。
    pub new_anger: i32,
}

impl PigZombieAngerEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_id: Option<i32>, new_anger: i32) -> Self {
        Self {
            entity_id,
            target_id,
            new_anger,
            cancelled: false,
        }
    }
}
