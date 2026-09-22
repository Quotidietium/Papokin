use std::sync::Arc;

use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetRedstonePowerArgs, OnPlaceArgs, OnScheduledTickArgs,
    OnStateReplacedArgs, PathComputationType,
};
use crate::world::World;
use papokin_data::block_properties::LightningRodLikeProperties;
use papokin_data::{BlockState, BlockStateId, FacingExt};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::tick::TickPriority;
use papokin_world::world::BlockFlags;

#[pumpkin_block("minecraft:lightning_rod")]
pub struct LightningRodBlock;

impl LightningRodBlock {
    pub fn trigger(world: &Arc<World>, pos: &BlockPos) {
        let (block, state_id) = world.get_block_and_state_id(pos);
        let mut props = LightningRodLikeProperties::from_state_id(state_id);
        if !props.powered {
            props.powered = true;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);

            Self::update_neighbors(world, pos, props);

            // 在原版中，它会保持充能 8 刻（4 个红石刻），直到计划刻将其关闭。
            world.schedule_block_tick(block, *pos, 8, TickPriority::Normal);
        }
    }

    fn update_neighbors(world: &Arc<World>, pos: &BlockPos, props: LightningRodLikeProperties) {
        world.update_neighbors(pos, None);
        // 它所依附的方块位于朝向的反方向
        let attached_pos = pos.offset(props.facing.opposite().to_block_direction().to_offset());
        world.update_neighbors(&attached_pos, None);
    }
}

impl BlockBehaviour for LightningRodBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = LightningRodLikeProperties::default(args.block);
        props.facing = args.direction.to_facing().opposite();
        props.to_state_id(args.block)
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LightningRodLikeProperties::from_state_id(args.state.id);
        if props.powered { 15 } else { 0 }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LightningRodLikeProperties::from_state_id(args.state.id);
        // 它只在其朝向方向（指向外侧的方向）发出强充能
        if props.powered && props.facing.to_block_direction() == args.direction {
            15
        } else {
            0
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = LightningRodLikeProperties::from_state_id(state.id);
        if props.powered {
            props.powered = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            Self::update_neighbors(args.world, args.position, props);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let props = LightningRodLikeProperties::from_state_id(args.old_state_id);
            if props.powered {
                Self::update_neighbors(args.world, args.position, props);
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
