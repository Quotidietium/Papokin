use papokin_macros::{Event, cancellable};

/// 实体将另一个实体设为目标时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTargetEvent {
    /// 正在指定目标的实体的 ID。
    pub entity_id: i32,

    /// 目标实体的 ID（如有）。
    pub target_id: Option<i32>,
}

impl EntityTargetEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_id: Option<i32>) -> Self {
        Self {
            entity_id,
            target_id,
            cancelled: false,
        }
    }
}
