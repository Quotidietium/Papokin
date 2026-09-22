use papokin_macros::{Event, cancellable};

/// 末影龙改变阶段时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EnderDragonChangePhaseEvent {
    /// 末影龙的 ID。
    pub entity_id: i32,

    /// 末影龙当前的阶段。
    pub current_phase: String,

    /// 末影龙的新阶段。
    pub new_phase: String,
}
