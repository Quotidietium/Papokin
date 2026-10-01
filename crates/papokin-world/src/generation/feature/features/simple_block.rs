use papokin_data::Block;
use papokin_util::{math::position::BlockPos, random::RandomGenerator};

use crate::generation::proto_chunk::GenerationCache;
use crate::{
    generation::block_state_provider::BlockStateProvider,
    world::{BlockAccessor, WorldPortalExt},
};

pub struct SimpleBlockFeature {
    pub to_place: BlockStateProvider,
    pub schedule_tick: Option<bool>,
}

impl SimpleBlockFeature {
    pub fn generate<T: GenerationCache>(
        &self,
        block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let state = self.to_place.get(random, pos, chunk, block_registry);
        let block = Block::from_state_id(state.id);
        let block_accessor: &dyn BlockAccessor = chunk;
        if !block_registry.can_place_at(block, state, block_accessor, &pos) {
            return false;
        }

        chunk.set_block_state(&pos.0, state);
        // 双层植物（向日葵/高草/高大海草/小垂滴叶等）的 simple_block 地物
        // 只落下半态，且生成路径不经过 placed 行为钩子——上半伴随方块须在
        // 此显式补齐（原版 placeAt 语义；can_place_at 已保证上方可占据）。
        let block_accessor: &dyn BlockAccessor = chunk;
        for (extra_pos, extra_state_id) in
            block_registry.extra_generation_blocks(block, &pos, state.id, block_accessor)
        {
            chunk.set_block_state(
                &extra_pos.0,
                papokin_data::BlockState::from_id(extra_state_id),
            );
        }
        true
    }
}
