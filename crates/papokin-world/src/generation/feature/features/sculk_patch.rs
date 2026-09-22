//! 幽匿斑块（sculk patch）已配置地物的驱动器。
//!
//! 此模块保留所需的公开结构体 / 字段布局，以满足
//! 代码生成（`configured_features_generated.rs`）以及两个生成入口
//! 点（`generate` / `generate_in_proto_chunk`）。实际的蔓延
//! 算法位于 [`sculk`] 子模块中。

use papokin_data::Block;
use papokin_util::math::int_provider::IntProvider;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_util::random::RandomGenerator;
use papokin_util::random::RandomImpl;

use crate::generation::feature::features::sculk;
use crate::generation::feature::features::sculk::ProtoChunkSculkView;
use crate::generation::feature::features::sculk::SculkLevel;
use crate::generation::feature::features::sculk::spreader::SculkSpreader;
use crate::generation::proto_chunk::GenerationCache;
use crate::world::WorldPortalExt;

pub struct SculkPatchFeature {
    pub charge_count: i32,
    pub amount_per_charge: i32,
    pub spread_attempts: i32,
    pub growth_rounds: i32,
    pub spread_rounds: i32,
    pub extra_rare_growths: IntProvider,
    pub catalyst_chance: f32,
}

impl SculkPatchFeature {
    /// 地形生成后的入口点。`T: GenerationCache` 同时
    /// 通过 blanket impl 实现 `SculkLevel`。
    pub fn generate<T: GenerationCache>(
        &self,
        _block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        if !sculk::can_spread_from(chunk, pos) {
            return false;
        }

        let mut spreader = SculkSpreader::new_world_gen_spreader();
        let total_rounds = self.spread_rounds + self.growth_rounds;

        for round in 0..total_rounds {
            for _ in 0..self.charge_count {
                spreader.add_cursors(pos, self.amount_per_charge);
            }

            for _ in 0..self.spread_attempts {
                spreader.update_cursors(chunk, pos, random, round < self.spread_rounds);
            }

            spreader.clear();
        }

        // 催化剂下方需要一个完整方块。
        if random.next_f32() <= self.catalyst_chance && chunk.sculk_is_full_cube(pos.down()) {
            chunk.sculk_set(pos, Block::SCULK_CATALYST.default_state);
        }

        // 极稀有的生长物（尖啸体，仅见于远古城市）
        self.place_extra_rare_growths(chunk, random, pos);

        true
    }

    /// 区块内（proto-chunk）生成的入口点，在地形生成阶段使用。
    pub fn generate_in_proto_chunk(
        &self,
        chunk: &mut crate::ProtoChunk,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let mut view = ProtoChunkSculkView::new(chunk);
        if !sculk::can_spread_from(&view, pos) {
            return false;
        }

        let mut spreader = SculkSpreader::new_world_gen_spreader();
        let total_rounds = self.spread_rounds + self.growth_rounds;
        for round in 0..total_rounds {
            for _ in 0..self.charge_count {
                spreader.add_cursors(pos, self.amount_per_charge);
            }
            for _ in 0..self.spread_attempts {
                spreader.update_cursors(&mut view, pos, random, round < self.spread_rounds);
            }
            spreader.clear();
        }

        if random.next_f32() <= self.catalyst_chance && view.sculk_is_full_cube(pos.down()) {
            view.sculk_set(pos, Block::SCULK_CATALYST.default_state);
        }

        self.place_extra_rare_growths(&mut view, random, pos);

        true
    }

    /// 在原点周围放置极为罕见的生长物（幽匿尖啸体）。
    fn place_extra_rare_growths(
        &self,
        level: &mut dyn SculkLevel,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) {
        let extra_growths = self.extra_rare_growths.get(random);
        for _ in 0..extra_growths {
            let candidate = pos.offset(Vector3::new(
                random.next_bounded_i32(5) - 2,
                0,
                random.next_bounded_i32(5) - 2,
            ));
            if level.sculk_is_air(candidate)
                && level.sculk_is_face_sturdy(candidate.down(), papokin_data::BlockDirection::Up)
            {
                level.sculk_set(candidate, sculk::shrieker_state(true));
            }
        }
    }
}
