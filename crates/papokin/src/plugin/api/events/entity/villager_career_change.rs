use papokin_macros::{Event, cancellable};

/// 村民更改职业时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VillagerCareerChangeEvent {
    /// 村民实体的 ID。
    pub entity_id: i32,

    /// 新的职业名称。
    pub profession: String,

    /// 变更的原因。
    pub reason: String,
}
