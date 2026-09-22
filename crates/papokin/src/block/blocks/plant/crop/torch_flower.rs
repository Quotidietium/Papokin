use papokin_data::Block;
use papokin_data::BlockStateId;
use papokin_data::block_properties::TorchflowerCropLikeProperties;
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockAccessor;
use rand::RngExt;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::blocks::plant::crop::CropBlockBase;
use crate::block::{BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, RandomTickArgs};

type TorchFlowerProperties = TorchflowerCropLikeProperties;

#[pumpkin_block("minecraft:torchflower_crop")]
pub struct TorchFlowerBlock;

impl BlockBehaviour for TorchFlowerBlock {
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
        if rand::rng().random_range(0..2) != 0 {
            <Self as CropBlockBase>::random_tick(self, args.world, args.position);
        }
    }
}

impl PlantBlockBase for TorchFlowerBlock {
    // 作物要求下方为耕地；没有此覆写，通用植物
    // 存活检查（`supports_vegetation`）能让它们在泥土上存活。
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        <Self as CropBlockBase>::can_plant_crop_on_top(self, block_accessor, pos)
    }
}

impl CropBlockBase for TorchFlowerBlock {
    fn bonemeal_age_increase(&self) -> i32 {
        1
    }

    fn max_age(&self) -> i32 {
        2
    }

    fn get_age(&self, state: BlockStateId, _block: &Block) -> i32 {
        let props = TorchFlowerProperties::from_state_id(state);
        i32::from(props.age)
    }

    fn state_with_age(&self, block: &Block, state: BlockStateId, age: i32) -> BlockStateId {
        if age == 1 {
            let mut properties = TorchFlowerProperties::from_state_id(state);
            properties.age = 1;
            properties.to_state_id(block)
        } else {
            Block::TORCHFLOWER.default_state.id
        }
    }
}
