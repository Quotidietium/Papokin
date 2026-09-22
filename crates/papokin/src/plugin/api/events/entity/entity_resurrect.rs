use papokin_macros::{Event, cancellable};

/// 实体被不死图腾从死亡中拯救时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityResurrectEvent {
    /// 被复活实体的 ID。
    pub entity_id: i32,
}

impl EntityResurrectEvent {
    #[must_use]
    pub const fn new(entity_id: i32) -> Self {
        Self {
            entity_id,
            cancelled: false,
        }
    }
}
