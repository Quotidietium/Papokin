use papokin_macros::{Event, cancellable};

/// 区域效果云对实体施加效果时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct AreaEffectCloudApplyEvent {
    /// 区域效果云的实体 ID。
    pub entity_id: i32,
    /// 受影响实体的 ID 列表。
    pub affected_entities: Vec<i32>,
}

impl AreaEffectCloudApplyEvent {
    #[must_use]
    pub const fn new(entity_id: i32, affected_entities: Vec<i32>) -> Self {
        Self {
            entity_id,
            affected_entities,
            cancelled: false,
        }
    }
}
