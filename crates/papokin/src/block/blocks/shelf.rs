use papokin_data::HorizontalFacingExt;
use papokin_data::block_properties::AcaciaShelfLikeProperties;
use papokin_data::{BlockState, BlockStateId};
use papokin_inventory::Inventory;
use papokin_macros::pumpkin_block_from_tag;

use crate::block::entities::shelf::ShelfBlockEntity;
use crate::block::{
    BlockBehaviour, GetComparatorOutputArgs, OnPlaceArgs, PathComputationType, PlacedArgs,
};
use crate::entity::EntityBase;
use std::sync::Arc;

#[pumpkin_block_from_tag("minecraft:wooden_shelves")]
pub struct ShelfBlock;

impl BlockBehaviour for ShelfBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut properties = AcaciaShelfLikeProperties::default(args.block);

        properties.facing = args.player.get_entity().get_horizontal_facing().opposite();

        properties.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = ShelfBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        // 原版只通过架子的背面读取。
        let props = AcaciaShelfLikeProperties::from_state_id(args.state.id);
        if args.direction != props.facing.to_block_direction().opposite() {
            return Some(0);
        }

        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(shelf) = block_entity.as_any().downcast_ref::<ShelfBlockEntity>()
        {
            // 借用探测：比较器输出只需 is_empty，深克隆白付
            let mut bits = 0u8;
            for (i, bit) in (0..3).zip([1u8, 2, 4]) {
                let mut occupied = false;
                shelf.with_stack(i, &mut |stack| occupied = !stack.is_empty());
                if occupied {
                    bits |= bit;
                }
            }
            Some(bits)
        } else {
            None
        }
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        computation_type == PathComputationType::Water && state.is_waterlogged()
    }
}
