use papokin_macros::{Event, cancellable};

/// 监守者对某个实体的怒气变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WardenAngerChangeEvent {
    /// 监守者实体的 ID。
    pub entity_id: i32,

    /// 目标实体的 ID。
    pub target_id: i32,

    /// 之前的愤怒等级。
    pub old_anger: i32,

    /// 新的愤怒等级。
    pub new_anger: i32,
}
