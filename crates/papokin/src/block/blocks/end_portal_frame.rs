use papokin_data::{BlockState, BlockStateId};
use papokin_macros::pumpkin_block;

use crate::{
    block::{BlockBehaviour, GetComparatorOutputArgs, OnPlaceArgs, PathComputationType},
    entity::EntityBase,
};

type EndPortalFrameProperties = papokin_data::block_properties::EndPortalFrameLikeProperties;

#[pumpkin_block("minecraft:end_portal_frame")]
pub struct EndPortalFrameBlock;

impl BlockBehaviour for EndPortalFrameBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut end_portal_frame_props = EndPortalFrameProperties::default(args.block);
        end_portal_frame_props.facing = args.player.get_entity().get_horizontal_facing().opposite();

        end_portal_frame_props.to_state_id(args.block)
    }

    /// 装有末影之眼的框架读取为满强度。
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = EndPortalFrameProperties::from_state_id(args.state.id);
        Some(if props.eye { 15 } else { 0 })
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
