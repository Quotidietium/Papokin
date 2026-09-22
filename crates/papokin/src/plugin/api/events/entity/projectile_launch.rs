use papokin_macros::{Event, cancellable};

/// 弹射物被发射时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ProjectileLaunchEvent {
    /// 投射物实体的 ID。
    pub entity_id: i32,
    /// 射击者实体的 ID（如适用）。
    pub shooter_id: Option<i32>,
}

impl ProjectileLaunchEvent {
    #[must_use]
    pub const fn new(entity_id: i32, shooter_id: Option<i32>) -> Self {
        Self {
            entity_id,
            shooter_id,
            cancelled: false,
        }
    }
}
