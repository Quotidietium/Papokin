use papokin_data::BlockStateId;
use papokin_data::tag::{self, Taggable};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};

use crate::block::{BlockBehaviour, CanPlaceAtArgs, OnEntityCollisionArgs};

#[pumpkin_block("minecraft:lily_pad")]
pub struct LilyPadBlock;

impl BlockBehaviour for LilyPadBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        {
            // 可能不是最佳方案，但能用
            if args
                .entity
                .get_entity()
                .entity_type
                .resource_name
                .ends_with("_boat")
            {
                args.world
                    .break_block(args.position, None, BlockFlags::empty());
            }
        }
    }

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
}

impl PlantBlockBase for LilyPadBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        // TODO: 获取并使用流体而非方块
        let block = block_accessor.get_block(pos);
        let above_fluid = block_accessor.get_block(&pos.up());
        (block.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_LILY_PAD)
            || block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_LILY_PAD))
            && above_fluid.is_air()
    }
}
