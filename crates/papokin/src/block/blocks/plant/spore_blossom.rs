use crate::block::{BlockBehaviour, CanPlaceAtArgs};
use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};
use papokin_data::BlockStateId;
use papokin_data::tag::Taggable;
use papokin_data::{Block, tag};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockAccessor;

#[pumpkin_block("minecraft:spore_blossom")]
pub struct SporeBlossomBlock;

impl BlockBehaviour for SporeBlossomBlock {
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
impl PlantBlockBase for SporeBlossomBlock {
    fn can_plant_on_top(
        &self,
        _block_accessor: &dyn papokin_world::world::BlockAccessor,
        _pos: &papokin_util::math::position::BlockPos,
    ) -> bool {
        false
    }
    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        let ceiling_block = block_accessor.get_block(&block_pos.up());
        supports_spore_blossom(ceiling_block)
    }
}
fn supports_spore_blossom(block: &Block) -> bool {
    !block.has_tag(&tag::Block::MINECRAFT_LEAVES) && block.is_solid()
}
