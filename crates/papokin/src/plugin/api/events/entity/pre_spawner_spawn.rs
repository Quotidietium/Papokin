use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 刷怪笼生成实体前发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PreSpawnerSpawnEvent {
    /// 刷怪笼方块的位置。
    pub spawner_pos: BlockPos,

    /// 正在生成的实体类型的标识符（例如 `minecraft:zombie`）。
    pub entity_type: String,
}

impl PreSpawnerSpawnEvent {
    #[must_use]
    pub const fn new(spawner_pos: BlockPos, entity_type: String) -> Self {
        Self {
            spawner_pos,
            entity_type,
            cancelled: false,
        }
    }
}
