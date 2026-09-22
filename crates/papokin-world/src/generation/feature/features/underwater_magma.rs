use crate::generation::proto_chunk::GenerationCache;
use papokin_data::{Block, BlockDirection, BlockState};
use papokin_util::{
    math::position::BlockPos,
    random::{RandomGenerator, RandomImpl},
};

pub struct UnderwaterMagmaFeature {
    /// 从原点出发寻找水柱底部的垂直搜索上限
    pub floor_search_range: i32,
    /// 从落点起用于采样的水平半径
    pub placement_radius: i32,
    /// 每个被检方块转化为岩浆的概率
    pub placement_probability: f32,
}

impl UnderwaterMagmaFeature {
    /// 在原点的 XZ 处查找水柱下方实体地面的 Y 坐标。
    fn get_floor_y<T: GenerationCache>(&self, chunk: &T, origin: BlockPos) -> Option<i32> {
        let x = origin.0.x;
        let z = origin.0.z;

        // 向下直到找到水
        for dy in 0..=self.floor_search_range {
            let check_y = origin.0.y - dy;
            let pos = BlockPos::new(x, check_y, z);
            let state_id = GenerationCache::get_block_state(chunk, &pos.0).to_block_id();

            if state_id == Block::WATER {
                // 下沉到该列下方第一个非水方块
                let mut floor_y = check_y - 1;
                loop {
                    let floor_pos = BlockPos::new(x, floor_y, z);
                    let floor_id =
                        GenerationCache::get_block_state(chunk, &floor_pos.0).to_block_id();
                    if floor_id != Block::WATER {
                        return Some(floor_y);
                    }
                    if (check_y - floor_y) > self.floor_search_range {
                        break;
                    }
                    floor_y -= 1;
                }
            }
        }
        None
    }

    /// 检查方块能否承载岩浆块
    fn is_valid_placement<T: GenerationCache>(chunk: &T, target: &BlockPos) -> bool {
        // 拒绝水/空气或不支持的方块
        let target_id = GenerationCache::get_block_state(chunk, &target.0).to_block_id();
        if target_id == Block::WATER || target_id == Block::AIR {
            return false;
        }

        // 下方必须是实心方块
        let below = target.offset(BlockDirection::Down.to_offset());
        let below_id = GenerationCache::get_block_state(chunk, &below.0).to_block_id();
        if below_id == Block::WATER || below_id == Block::AIR {
            return false;
        }

        // 无开放的横向面
        for dir in &BlockDirection::horizontal() {
            let neighbour = target.offset(dir.to_offset());
            let n_id = GenerationCache::get_block_state(chunk, &neighbour.0).to_block_id();
            if n_id == Block::WATER || n_id == Block::AIR {
                return false;
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate<T: GenerationCache>(
        &self,
        chunk: &mut T,
        _min_y: i8,
        _height: u16,
        _feature_name: papokin_data::placed_feature::PlacedFeature,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        // 定位地板；若无则中止
        let Some(floor_y) = self.get_floor_y(chunk, pos) else {
            return false;
        };

        let floor_pos = BlockPos::new(pos.0.x, floor_y, pos.0.z);

        // 围绕地面采样一个立方体区域，按概率与有效性检查放置岩浆块
        let mut placed = 0i32;
        let r = self.placement_radius;

        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if random.next_f32() >= self.placement_probability {
                        continue;
                    }

                    let target_pos =
                        BlockPos::new(floor_pos.0.x + dx, floor_pos.0.y + dy, floor_pos.0.z + dz);

                    if !Self::is_valid_placement(chunk, &target_pos) {
                        continue;
                    }

                    let magma_state = BlockState::from_id(Block::MAGMA_BLOCK.default_state.id);
                    chunk.set_block_state(&target_pos.0, magma_state);
                    placed += 1;
                }
            }
        }

        placed > 0
    }
}
