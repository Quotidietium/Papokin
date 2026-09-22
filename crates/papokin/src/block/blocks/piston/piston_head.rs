use papokin_data::block_properties::Facing;
use papokin_data::{Block, BlockState, FacingExt};
use papokin_macros::pumpkin_block;
use papokin_world::world::BlockFlags;

use crate::block::blocks::piston::piston::try_move;
use crate::block::{BlockBehaviour, BrokenArgs, OnNeighborUpdateArgs, PathComputationType};

use super::piston::PistonProps;

pub(crate) type PistonHeadProperties = papokin_data::block_properties::PistonHeadLikeProperties;

#[pumpkin_block("minecraft:piston_head")]
pub struct PistonHeadBlock;

impl BlockBehaviour for PistonHeadBlock {
    fn broken(&self, args: BrokenArgs<'_>) {
        let props = PistonHeadProperties::from_state_id(args.state.id);
        let pos = args
            .position
            .offset(props.facing.opposite().to_block_direction().to_offset());
        let (new_block, new_state) = args.world.get_block_and_state_id(&pos);
        if &Block::PISTON == new_block || &Block::STICKY_PISTON == new_block {
            let props = PistonProps::from_state_id(new_state);
            if props.extended {
                // TODO: 使用玩家
                args.world.break_block(&pos, None, BlockFlags::SKIP_DROPS);
            }
        }
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let head_state_id = args.world.get_block_state_id(args.position);
        let head_props = PistonHeadProperties::from_state_id(head_state_id);
        if head_props.facing != Facing::Up {
            return;
        }
        let piston_pos = args.position.offset(
            head_props
                .facing
                .opposite()
                .to_block_direction()
                .to_offset(),
        );
        let piston_block = args.world.get_block(&piston_pos);
        if &Block::PISTON == piston_block || &Block::STICKY_PISTON == piston_block {
            let up_pos = args
                .position
                .offset(head_props.facing.to_block_direction().to_offset());
            let upper_block = args.world.get_block(&up_pos);
            if upper_block != &Block::REDSTONE_BLOCK {
                //那么很可能是有人破坏了红石块，尝试检查活塞是否仍应保持伸出。
                try_move(args.world, piston_block, &piston_pos);
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
