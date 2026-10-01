use crate::block::{BrokenArgs, PlacedArgs};
use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::BlockId;
use papokin_data::BlockStateId;
use papokin_data::block_properties::{DoubleBlockHalf, TallSeagrassLikeProperties};
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::PlantBlockBase,
};

pub struct TallPlantBlock;

impl BlockMetadata for TallPlantBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::TALL_GRASS,
            BlockId::LARGE_FERN,
            BlockId::PITCHER_PLANT,
            // TallFlowerBlocks
            BlockId::SUNFLOWER,
            BlockId::LILAC,
            BlockId::PEONY,
            BlockId::ROSE_BUSH,
        ]
        .into()
    }
}

impl BlockBehaviour for TallPlantBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let up_pos = args.position.up();

        let upper_state = args.block_accessor.get_block_state(&up_pos);
        let Some(world) = args.world else {
            return <Self as PlantBlockBase>::can_place_at(
                self,
                args.block_accessor,
                args.position,
            ) && upper_state.is_air();
        };

        if up_pos.0.y > world.get_top_y() {
            return false;
        }
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
            && upper_state.is_air()
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let tall_plant_props = TallSeagrassLikeProperties::from_state_id(args.state_id);
        let (support_block_pos, other_block_pos) = match tall_plant_props.half {
            DoubleBlockHalf::Upper => (args.position.down_height(2), args.position.down()),
            DoubleBlockHalf::Lower => (args.position.down(), args.position.up()),
        };
        if !<Self as PlantBlockBase>::can_place_at(self, args.world, &support_block_pos.up()) {
            return Block::AIR.default_state.id;
        }

        let (other_block, other_state_id) = args.world.get_block_and_state_id(&other_block_pos);
        if Self::ids().contains(&other_block.id) {
            let other_props = TallSeagrassLikeProperties::from_state_id(other_state_id);
            let opposite_half = match tall_plant_props.half {
                DoubleBlockHalf::Upper => DoubleBlockHalf::Lower,
                DoubleBlockHalf::Lower => DoubleBlockHalf::Upper,
            };
            if other_props.half == opposite_half {
                return args.state_id;
            }
        }
        Block::AIR.default_state.id
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let mut tall_plant_props = TallSeagrassLikeProperties::from_state_id(args.state_id);
            tall_plant_props.half = DoubleBlockHalf::Upper;
            args.world.set_block_state(
                &args.position.offset(BlockDirection::Up.to_offset()),
                tall_plant_props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK,
            );
        }
    }

    fn extra_placed_blocks(
        &self,
        _world: &crate::world::World,
        block: &Block,
        position: &BlockPos,
        state_id: BlockStateId,
    ) -> Vec<(BlockPos, BlockStateId)> {
        Self::upper_pair(block, position, state_id)
            .into_iter()
            .collect()
    }

    fn extra_generation_blocks(
        &self,
        block: &Block,
        position: &BlockPos,
        state_id: BlockStateId,
    ) -> Vec<(BlockPos, BlockStateId)> {
        Self::upper_pair(block, position, state_id)
            .into_iter()
            .collect()
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        {
            // 当高大植物的一半被破坏时，另一半也要一并破坏
            let tall_plant_props = TallSeagrassLikeProperties::from_state_id(args.state.id);
            let other_block_pos = match tall_plant_props.half {
                DoubleBlockHalf::Upper => args.position.down(),
                DoubleBlockHalf::Lower => args.position.up(),
            };
            let (other_block, other_state_id) = args.world.get_block_and_state_id(&other_block_pos);
            if Self::ids().contains(&other_block.id) {
                let other_props = TallSeagrassLikeProperties::from_state_id(other_state_id);
                let opposite_half = match tall_plant_props.half {
                    DoubleBlockHalf::Upper => DoubleBlockHalf::Lower,
                    DoubleBlockHalf::Lower => DoubleBlockHalf::Upper,
                };
                if other_props.half == opposite_half {
                    // 破坏另一半，使用 SKIP_DROPS 防止重复掉落
                    args.world.break_block(
                        &other_block_pos,
                        None,
                        BlockFlags::SKIP_DROPS
                            | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK
                            | BlockFlags::NOTIFY_ALL,
                    );
                }
            }
        }
    }
}

impl TallPlantBlock {
    /// 上半伴随方块：同方块、`half` 翻转为 Upper（原版 `placeAt`
    /// 语义的静态半边）；仅对下半态返回，防止对上半态再翻一次。
    fn upper_pair(
        block: &Block,
        position: &BlockPos,
        state_id: BlockStateId,
    ) -> Option<(BlockPos, BlockStateId)> {
        let mut tall_plant_props = TallSeagrassLikeProperties::from_state_id(state_id);
        if tall_plant_props.half != DoubleBlockHalf::Lower {
            return None;
        }
        tall_plant_props.half = DoubleBlockHalf::Upper;
        Some((
            position.offset(BlockDirection::Up.to_offset()),
            tall_plant_props.to_state_id(block),
        ))
    }
}

impl PlantBlockBase for TallPlantBlock {}

#[cfg(test)]
mod tests {
    use papokin_data::block_properties::DoubleBlockHalf;
    use papokin_data::{Block, BlockId};

    use super::TallPlantBlock;
    use crate::block::{BlockBehaviour, BlockMetadata};
    use papokin_util::math::position::BlockPos;

    /// 全部双层植物（向日葵/高草/丁香/玫瑰丛/牡丹/瓶子草/大蕨）的
    /// 生成期伴随方块：恰好一格上方、同方块、`half=Upper`。世界生成的
    /// `simple_block` 地物只落下半态，缺失此伴随方块时客户端只看到
    /// 半株植物（2026-10-02 向日葵只有下半方块的根因）。
    #[test]
    fn every_tall_plant_yields_exactly_one_upper_companion() {
        let pos = BlockPos::new(3, 64, 5);
        for block in [
            Block::SUNFLOWER,
            Block::TALL_GRASS,
            Block::LARGE_FERN,
            Block::LILAC,
            Block::PEONY,
            Block::ROSE_BUSH,
            Block::PITCHER_PLANT,
        ] {
            let lower = block.default_state;
            assert_eq!(
                super::TallSeagrassLikeProperties::from_state_id(lower.id).half,
                DoubleBlockHalf::Lower,
                "{} 的默认态应为下半",
                block.name
            );
            let extras = TallPlantBlock.extra_generation_blocks(&block, &pos, lower.id);
            assert_eq!(extras.len(), 1, "{} 应恰好有一个上半伴随方块", block.name);
            let (extra_pos, extra_state) = extras[0];
            assert_eq!(extra_pos, pos.up(), "{} 伴随方块应在其上方", block.name);
            assert_eq!(
                super::TallSeagrassLikeProperties::from_state_id(extra_state).half,
                DoubleBlockHalf::Upper,
                "{} 伴随方块应为上半态",
                block.name
            );
            assert_eq!(
                Block::from_state_id(extra_state).id,
                block.id,
                "{} 伴随方块应为同一方块",
                block.name
            );
            // 上半态不应再产出伴随（防递归翻转）。
            assert!(
                TallPlantBlock
                    .extra_generation_blocks(&block, &extra_pos, extra_state)
                    .is_empty(),
                "{} 上半态不得再产出伴随方块",
                block.name
            );
        }
    }

    /// 行为类覆盖的方块集合与数据集方块 id 的一致性锚点（新增双层
    /// 植物时两边都要同步）。
    #[test]
    fn tall_plant_ids_cover_expected_set() {
        let ids: Vec<BlockId> = TallPlantBlock::ids().into();
        for expected in [
            BlockId::TALL_GRASS,
            BlockId::LARGE_FERN,
            BlockId::PITCHER_PLANT,
            BlockId::SUNFLOWER,
            BlockId::LILAC,
            BlockId::PEONY,
            BlockId::ROSE_BUSH,
        ] {
            assert!(ids.contains(&expected), "缺少双层植物 {expected:?}");
        }
        assert_eq!(ids.len(), 7);
    }
}
