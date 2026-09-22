use papokin_macros::{Event, cancellable};

/// 村民对玩家的声誉变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VillagerReputationChangeEvent {
    /// 村民实体 ID。
    pub entity_id: i32,
    /// 目标玩家实体 ID。
    pub target_id: i32,
    /// 声望变化量。
    pub reputation_change: i32,
}

impl VillagerReputationChangeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_id: i32, reputation_change: i32) -> Self {
        Self {
            entity_id,
            target_id,
            reputation_change,
            cancelled: false,
        }
    }
}
