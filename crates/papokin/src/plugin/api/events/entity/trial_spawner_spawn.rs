use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 试炼刷怪笼生成实体时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct TrialSpawnerSpawnEvent {
    /// 已生成实体的 ID。
    pub entity_id: i32,
    /// 试炼刷怪笼方块位置。
    pub spawner_pos: BlockPos,
}

impl TrialSpawnerSpawnEvent {
    #[must_use]
    pub const fn new(entity_id: i32, spawner_pos: BlockPos) -> Self {
        Self {
            entity_id,
            spawner_pos,
            cancelled: false,
        }
    }
}
