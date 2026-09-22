use papokin_macros::{Event, cancellable};
use papokin_util::math::vector2::Vector2;

/// 区块被填充/生成时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ChunkPopulateEvent {
    /// 区块坐标。
    pub chunk_pos: Vector2<i32>,
}

impl ChunkPopulateEvent {
    #[must_use]
    pub const fn new(chunk_pos: Vector2<i32>) -> Self {
        Self {
            chunk_pos,
            cancelled: false,
        }
    }
}
