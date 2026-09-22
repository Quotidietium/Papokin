use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 刷怪笼生成实体时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SpawnerSpawnEvent {
    /// 已生成实体的 ID。
    pub entity_id: i32,
    /// 刷怪笼的方块位置。
    pub spawner_pos: BlockPos,
}

impl SpawnerSpawnEvent {
    #[must_use]
    pub const fn new(entity_id: i32, spawner_pos: BlockPos) -> Self {
        Self {
            entity_id,
            spawner_pos,
            cancelled: false,
        }
    }
}
