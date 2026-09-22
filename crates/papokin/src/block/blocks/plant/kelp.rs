use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    PlacedArgs,
};
use papokin_data::BlockStateId;
use papokin_data::block_properties::WaterLikeProperties;
use papokin_data::tag::Taggable;
use papokin_data::{Block, BlockId, tag};
use papokin_util::math::position::BlockPos;
use papokin_world::world::{BlockAccessor, BlockFlags};
pub struct KelpBlock;

impl BlockMetadata for KelpBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::KELP, BlockId::KELP_PLANT].into()
    }
}

impl BlockBehaviour for KelpBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
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
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::KELP {
                args.world.set_block_state(
                    &support_pos,
                    Block::KELP_PLANT.default_state.id,
                    BlockFlags::empty(),
                );
            }
        }
    }
    fn broken(&self, args: BrokenArgs<'_>) {
        {
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::KELP_PLANT {
                args.world.set_block_state(
                    &support_pos,
                    Block::KELP.default_state.id,
                    BlockFlags::empty(),
                );
                args.world.set_block_state(
                    args.position,
                    Block::WATER.default_state.id,
                    BlockFlags::empty(),
                );
            }
        }
    }
}

impl PlantBlockBase for KelpBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn papokin_world::world::BlockAccessor,
        pos: &papokin_util::math::position::BlockPos,
    ) -> bool {
        // 确定支撑方块
        let support_pos = pos;
        let (replacing_block, replacing_block_state) =
            block_accessor.get_block_and_state(&pos.up());
        let (support_block, support_block_state) = block_accessor.get_block_and_state(support_pos);
        if replacing_block == &Block::WATER {
            let water_props = WaterLikeProperties::from_state_id(replacing_block_state.id);

            //只允许将海带放置在完整水源或向下流动的水中
            if water_props.level != 0 && water_props.level != 8 {
                return false;
            }
        } else {
            //如果这是邻居更新检查，被替换的方块也可能是 kelp_plant 或 kelp
            if replacing_block != &Block::KELP_PLANT && replacing_block != &Block::KELP {
                return false;
            }
        }
        // 如果放置的是海带基部方块，则允许放置在水上或其他海带段上。
        if support_block == &Block::KELP || support_block == &Block::KELP_PLANT {
            return true;
        }
        if support_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_KELP) {
            return false;
        }
        if support_block_state.is_side_solid(papokin_data::BlockDirection::Up)
            && support_block.is_solid()
        {
            return true;
        }
        false
    }
    fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !<Self as PlantBlockBase>::can_place_at(self, block_accessor, block_pos) {
            return Block::WATER.default_state.id;
        }
        block_state
    }
}
