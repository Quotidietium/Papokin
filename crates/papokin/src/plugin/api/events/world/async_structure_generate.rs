use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 结构被异步生成时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncStructureGenerateEvent {
    pub world_name: String,
    pub structure_name: String,
    pub pos: BlockPos,
}

impl AsyncStructureGenerateEvent {
    #[must_use]
    pub const fn new(world_name: String, structure_name: String, pos: BlockPos) -> Self {
        Self {
            world_name,
            structure_name,
            pos,
            cancelled: false,
        }
    }
}
