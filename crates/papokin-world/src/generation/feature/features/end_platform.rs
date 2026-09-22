use papokin_data::Block;
use papokin_util::{
    math::{position::BlockPos, vector3::Vector3},
    random::RandomGenerator,
};

use crate::generation::proto_chunk::GenerationCache;
use crate::world::WorldPortalExt;

pub struct EndPlatformFeature;

impl EndPlatformFeature {
    pub fn generate<T: GenerationCache>(
        chunk: &mut T,
        _block_registry: &dyn WorldPortalExt,
        _min_y: i8,
        _height: u16,
        _feature: papokin_data::placed_feature::PlacedFeature, // 此放置的地物
        _random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                for dy in -1i32..3 {
                    let state = if dy == -1 {
                        Block::OBSIDIAN.default_state
                    } else {
                        Block::AIR.default_state
                    };
                    let target = Vector3::new(pos.0.x + dx, pos.0.y + dy, pos.0.z + dz);
                    chunk.set_block_state(&target, state);
                }
            }
        }
        true
    }
}
