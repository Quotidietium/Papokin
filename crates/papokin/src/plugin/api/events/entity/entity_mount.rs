use papokin_macros::{Event, cancellable};

/// 实体骑上另一个实体时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityMountEvent {
    /// 正在上坐骑的实体 ID。
    pub entity_id: i32,

    /// 被骑乘的载具实体的 ID。
    pub mount_id: i32,
}

impl EntityMountEvent {
    #[must_use]
    pub const fn new(entity_id: i32, mount_id: i32) -> Self {
        Self {
            entity_id,
            mount_id,
            cancelled: false,
        }
    }
}
