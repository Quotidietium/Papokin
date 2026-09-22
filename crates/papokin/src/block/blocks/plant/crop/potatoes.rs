use papokin_data::BlockStateId;
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockAccessor;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::blocks::plant::crop::CropBlockBase;
use crate::block::{BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, RandomTickArgs};

#[pumpkin_block("minecraft:potatoes")]
pub struct PotatoBlock;

impl BlockBehaviour for PotatoBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        <Self as CropBlockBase>::is_valid_bonemeal_target(self, args.world, args.position)
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        <Self as CropBlockBase>::perform_bonemeal(self, args.world, args.position);
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

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        <Self as CropBlockBase>::random_tick(self, args.world, args.position);
    }
}

impl PlantBlockBase for PotatoBlock {
    // 作物要求下方为耕地；没有此覆写，通用植物
    // 存活检查（`supports_vegetation`）能让它们在泥土上存活。
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        <Self as CropBlockBase>::can_plant_crop_on_top(self, block_accessor, pos)
    }
}

impl CropBlockBase for PotatoBlock {}
