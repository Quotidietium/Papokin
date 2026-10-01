use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs, PlacedArgs,
};
use papokin_data::BlockStateId;
use papokin_data::block_properties::{DoubleBlockHalf, SmallDripleafLikeProperties};
use papokin_data::tag::Taggable;
use papokin_data::{Block, tag};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::world::{BlockAccessor, BlockFlags};

#[pumpkin_block("minecraft:small_dripleaf")]
pub struct SmallDripleafBlock;

impl BlockBehaviour for SmallDripleafBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        let mut small_dripleaf_props = SmallDripleafLikeProperties::default(args.block);

        small_dripleaf_props.facing = facing;
        small_dripleaf_props.waterlogged = args.replacing.water_source();
        small_dripleaf_props.half = DoubleBlockHalf::Lower;

        small_dripleaf_props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let lower_small_dripleaf_props =
                SmallDripleafLikeProperties::from_state_id(args.state_id);
            if lower_small_dripleaf_props.half != DoubleBlockHalf::Lower {
                return;
            }

            let mut upper_small_dripleaf_props =
                SmallDripleafLikeProperties::default(&Block::SMALL_DRIPLEAF);

            let upper_block = args.world.get_block(&args.position.up());
            upper_small_dripleaf_props.facing = lower_small_dripleaf_props.facing;
            upper_small_dripleaf_props.waterlogged = upper_block == &Block::WATER;
            upper_small_dripleaf_props.half = DoubleBlockHalf::Upper;

            args.world.set_block_state(
                &args.position.up(),
                upper_small_dripleaf_props.to_state_id(&Block::SMALL_DRIPLEAF),
                BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK,
            );
        }
    }

    /// 生成期伴随方块：与 [`Self::placed`] 同构的上半（facing 沿袭下半、
    /// `waterlogged` 取上半位当前是否为水）。`dripleaf` 地物经
    /// `simple_block` 只落 `half=lower`，缺失此伴随方块时繁茂洞窟里
    /// 的小垂滴叶只剩下半。
    fn extra_generation_blocks(
        &self,
        block: &Block,
        position: &BlockPos,
        state_id: BlockStateId,
        block_accessor: &dyn BlockAccessor,
    ) -> Vec<(BlockPos, BlockStateId)> {
        let lower_props = SmallDripleafLikeProperties::from_state_id(state_id);
        if lower_props.half != DoubleBlockHalf::Lower {
            return Vec::new();
        }

        let mut upper_props = SmallDripleafLikeProperties::default(block);
        upper_props.facing = lower_props.facing;
        upper_props.waterlogged = block_accessor.get_block(&position.up()) == &Block::WATER;
        upper_props.half = DoubleBlockHalf::Upper;
        vec![(position.up(), upper_props.to_state_id(block))]
    }
}
fn is_small_dripleaf_waterlogged(state_id: BlockStateId) -> bool {
    let dripleaf_props = SmallDripleafLikeProperties::from_state_id(state_id);
    dripleaf_props.waterlogged
}
impl PlantBlockBase for SmallDripleafBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let support_block = block_accessor.get_block(pos);

        if support_block == &Block::SMALL_DRIPLEAF {
            return true;
        }
        let upper_block = block_accessor.get_block(&pos.up_height(2));
        if upper_block != &Block::AIR
            && upper_block != &Block::WATER
            && upper_block != &Block::SMALL_DRIPLEAF
        {
            return false;
        }
        let (replacing_block, replacing_block_state) =
            block_accessor.get_block_and_state(&pos.up());
        if replacing_block == &Block::SMALL_DRIPLEAF && replacing_block_state.is_waterlogged() {
            //用于处理邻居更新检查的情况
            supports_small_dripleaf(support_block, true)
        } else {
            supports_small_dripleaf(support_block, replacing_block == &Block::WATER)
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !<Self as PlantBlockBase>::can_place_at(self, block_accessor, block_pos) {
            if is_small_dripleaf_waterlogged(block_state) {
                return Block::WATER.default_state.id;
            }
            return Block::AIR.default_state.id;
        }
        let upper_block = block_accessor.get_block(&block_pos.up());
        let below_blow = block_accessor.get_block(&block_pos.down());
        if upper_block != &Block::SMALL_DRIPLEAF && below_blow != &Block::SMALL_DRIPLEAF {
            if is_small_dripleaf_waterlogged(block_state) {
                return Block::WATER.default_state.id;
            }
            return Block::AIR.default_state.id;
        }
        block_state
    }
}
fn supports_small_dripleaf(support_block: &Block, underwater: bool) -> bool {
    if support_block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_SMALL_DRIPLEAF) {
        return true;
    }
    underwater && support_block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_BIG_DRIPLEAF)
}

#[cfg(test)]
mod tests {
    use papokin_data::block_properties::DoubleBlockHalf;
    use papokin_data::{Block, BlockState, BlockStateId};
    use papokin_util::math::position::BlockPos;
    use papokin_world::world::BlockAccessor;

    use super::SmallDripleafBlock;
    use crate::block::BlockBehaviour;

    /// 指定一格为水、其余为空气的测试桩。
    struct WaterAtAccessor {
        water: BlockPos,
    }

    impl BlockAccessor for WaterAtAccessor {
        fn get_block(&self, position: &BlockPos) -> &'static Block {
            if *position == self.water {
                &Block::WATER
            } else {
                &Block::AIR
            }
        }

        fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
            if *position == self.water {
                Block::WATER.default_state
            } else {
                Block::AIR.default_state
            }
        }

        fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
            self.get_block_state(position).id
        }

        fn get_block_and_state(
            &self,
            position: &BlockPos,
        ) -> (&'static Block, &'static BlockState) {
            if *position == self.water {
                (&Block::WATER, Block::WATER.default_state)
            } else {
                (&Block::AIR, Block::AIR.default_state)
            }
        }
    }

    /// 生成期伴随方块：facing 沿袭下半、上半位为水时 `waterlogged=true`、
    /// `half` 翻转 Upper——`dripleaf` 地物经 `simple_block` 只落下半态，
    /// 缺失此伴随方块时繁茂洞窟里的小垂滴叶只剩下半。
    #[test]
    fn generation_companion_copies_facing_and_resolves_waterlogging() {
        let pos = BlockPos::new(5, 30, 7);
        let lower = Block::SMALL_DRIPLEAF.default_state;
        assert_eq!(
            super::SmallDripleafLikeProperties::from_state_id(lower.id).half,
            DoubleBlockHalf::Lower,
            "小垂滴叶默认态应为下半"
        );

        let extras = SmallDripleafBlock.extra_generation_blocks(
            &Block::SMALL_DRIPLEAF,
            &pos,
            lower.id,
            &WaterAtAccessor { water: pos.up() },
        );
        assert_eq!(extras.len(), 1, "下半态应恰有一个上半伴随方块");
        let (extra_pos, upper_id) = extras[0];
        assert_eq!(extra_pos, pos.up());

        let lower_props = super::SmallDripleafLikeProperties::from_state_id(lower.id);
        let upper_props = super::SmallDripleafLikeProperties::from_state_id(upper_id);
        assert_eq!(upper_props.half, DoubleBlockHalf::Upper);
        assert_eq!(
            upper_props.facing, lower_props.facing,
            "上半 facing 应沿袭下半"
        );
        assert!(
            upper_props.waterlogged,
            "上半位为水时 waterlogged 应为 true"
        );

        let (_, dry_upper_id) = SmallDripleafBlock.extra_generation_blocks(
            &Block::SMALL_DRIPLEAF,
            &pos,
            lower.id,
            &WaterAtAccessor {
                water: BlockPos::new(1000, 1000, 1000),
            },
        )[0];
        assert!(
            !super::SmallDripleafLikeProperties::from_state_id(dry_upper_id).waterlogged,
            "上半位为空气时 waterlogged 应为 false"
        );

        // 上半态不得再产出伴随（防递归）。
        assert!(
            SmallDripleafBlock
                .extra_generation_blocks(
                    &Block::SMALL_DRIPLEAF,
                    &pos.up(),
                    upper_id,
                    &WaterAtAccessor { water: pos.up() }
                )
                .is_empty()
        );
    }
}
