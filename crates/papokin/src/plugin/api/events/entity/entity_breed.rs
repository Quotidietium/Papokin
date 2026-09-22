use papokin_macros::{Event, cancellable};

/// 两个实体繁殖产生幼崽时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityBreedEvent {
    /// 父实体 1 的 ID。
    pub father_id: i32,

    /// 父实体 2 的 ID。
    pub mother_id: i32,

    /// 子实体的 ID。
    pub child_id: i32,
}

impl EntityBreedEvent {
    #[must_use]
    pub const fn new(father_id: i32, mother_id: i32, child_id: i32) -> Self {
        Self {
            father_id,
            mother_id,
            child_id,
            cancelled: false,
        }
    }
}
