use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::EntityBase, world::World};

/// 炼药锅流体等级发生变化的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CauldronChangeReason {
    BucketEmpty,
    BucketFill,
    BottleEmpty,
    BottleFill,
    NaturalFill,
    Extinguish,
    Unknown,
}

/// 炼药锅液位变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CauldronLevelChangeEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub old_level: i32,
    pub new_level: i32,
    pub reason: CauldronChangeReason,
    pub entity: Option<Arc<dyn EntityBase>>,
}

impl CauldronLevelChangeEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        old_level: i32,
        new_level: i32,
        reason: CauldronChangeReason,
        entity: Option<Arc<dyn EntityBase>>,
    ) -> Self {
        Self {
            block_pos,
            world,
            old_level,
            new_level,
            reason,
            entity,
            cancelled: false,
        }
    }
}
