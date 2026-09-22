use papokin_macros::{Event, cancellable};

/// 两个物品实体合并时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ItemMergeEvent {
    /// 主物品实体的 ID。
    pub entity_id: i32,

    /// 被合并进主物品的目标物品实体的 ID。
    pub target_id: i32,
}
