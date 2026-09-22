use papokin_macros::{Event, cancellable};

/// 实体使另一个实体解除骑乘时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDismountEvent {
    /// 正在下坐骑的实体 ID。
    pub entity_id: i32,

    /// 被下车的载具实体的 ID。
    pub dismounted_id: i32,
}

impl EntityDismountEvent {
    #[must_use]
    pub const fn new(entity_id: i32, dismounted_id: i32) -> Self {
        Self {
            entity_id,
            dismounted_id,
            cancelled: false,
        }
    }
}
