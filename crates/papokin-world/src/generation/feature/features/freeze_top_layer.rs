use crate::generation::proto_chunk::GenerationCache;
use papokin_data::block_properties::{BlockProperties, GrassBlockLikeProperties};
use papokin_data::tag;
use papokin_data::{Block, BlockId};
use papokin_util::math::position::BlockPos;
use papokin_util::random::RandomGenerator;

pub struct FreezeTopLayerFeature;

impl FreezeTopLayerFeature {
    pub fn generate<T: GenerationCache>(
        chunk: &mut T,
        _min_y: i8,
        _height: u16,
        _feature_name: papokin_data::placed_feature::PlacedFeature,
        _random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let origin_x = pos.0.x;
        let origin_z = pos.0.z;

        for dx in 0..16i32 {
            for dz in 0..16i32 {
                let x = origin_x + dx;
                let z = origin_z + dz;

                let y = chunk.top_motion_blocking_block_height_exclusive(x, z);
                let below_y = y - 1;

                let top_vec = BlockPos::new(x, y, z).0;
                let below_vec = BlockPos::new(x, below_y, z).0;
                let below = GenerationCache::get_block_state(chunk, &below_vec);
                let below_block = below.to_block_id();

                let biome = chunk.get_biome_for_terrain_gen(x, y, z);

                // 冻结检查
                if biome.weather.base_temperature() <= 0.15 && below_block == BlockId::WATER {
                    chunk.set_block_state(&below_vec, Block::ICE.default_state);
                    continue;
                }

                // 雪检查
                let top_temp =
                    biome
                        .weather
                        .compute_temperature(x as f64, y, z as f64, chunk.get_sea_level());

                if top_temp < 0.15 {
                    let top_raw = GenerationCache::get_block_state(chunk, &top_vec);
                    // topPos 必须是空气；belowPos 不能是空气（要有可站立的东西）
                    if top_raw.to_state().is_air()
                        && !below.to_state().is_air()
                        && !below_block.has_tag(tag::Block::MINECRAFT_CANNOT_SUPPORT_SNOW_LAYER)
                    {
                        chunk.set_block_state(&top_vec, Block::SNOW.default_state);

                        // 若下方方块带有 `snowy` 方块状态属性，则更新它
                        if GrassBlockLikeProperties::handles_block_id(below_block) {
                            let block = below_block.to_block();
                            let mut props = GrassBlockLikeProperties::from_state_id(below);
                            props.snowy = true;
                            chunk.set_block_state(&below_vec, props.to_state_id(block).to_state());
                        }
                    }
                }
            }
        }

        true
    }
}
