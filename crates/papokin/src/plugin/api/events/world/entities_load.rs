use papokin_macros::{Event, cancellable};
use papokin_util::math::vector2::Vector2;

/// 区块中的实体被加载时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntitiesLoadEvent {
    /// 区块坐标。
    pub chunk_pos: Vector2<i32>,
    /// 实体数量。
    pub entity_count: usize,
}

impl EntitiesLoadEvent {
    #[must_use]
    pub const fn new(chunk_pos: Vector2<i32>, entity_count: usize) -> Self {
        Self {
            chunk_pos,
            entity_count,
            cancelled: false,
        }
    }
}
