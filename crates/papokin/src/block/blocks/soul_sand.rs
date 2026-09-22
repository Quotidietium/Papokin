use papokin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, PathComputationType};

#[pumpkin_block("minecraft:soul_sand")]
pub struct SoulSandBlock;

impl BlockBehaviour for SoulSandBlock {
    fn is_pathfindable(
        &self,
        _state: &papokin_data::BlockState,
        _computation_type: PathComputationType,
    ) -> bool {
        false
    }
}
