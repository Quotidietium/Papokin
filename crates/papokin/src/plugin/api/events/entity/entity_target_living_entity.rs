use papokin_macros::{Event, cancellable};

/// 实体将一个生物实体设为目标时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTargetLivingEntityEvent {
    /// 正在指定目标的实体的 ID。
    pub entity_id: i32,

    /// 目标生物实体的 ID（如有）。
    pub target_id: Option<i32>,

    /// 锁定目标的原因。
    pub reason: String,
}
