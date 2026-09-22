use papokin_macros::{Event, cancellable};

/// 实体转化为另一个实体时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTransformEvent {
    /// 原实体的 ID。
    pub entity_id: i32,

    /// 转换后的新实体的 ID。
    pub new_entity_id: i32,

    /// 转化的原因。
    pub transform_reason: String,
}

impl EntityTransformEvent {
    #[must_use]
    pub const fn new(entity_id: i32, new_entity_id: i32, transform_reason: String) -> Self {
        Self {
            entity_id,
            new_entity_id,
            transform_reason,
            cancelled: false,
        }
    }
}
