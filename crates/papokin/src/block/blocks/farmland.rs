use std::sync::Arc;

use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, PathComputationType, RandomTickArgs,
};
use crate::world::World;
use papokin_data::block_properties::FarmlandLikeProperties;
use papokin_data::tag;
use papokin_data::tag::Taggable;
use papokin_data::{Block, BlockDirection, BlockState, BlockStateId};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_world::tick::TickPriority;
use papokin_world::world::BlockAccessor;
use papokin_world::world::BlockFlags;

type FarmlandProperties = FarmlandLikeProperties;

#[pumpkin_block("minecraft:farmland")]
pub struct FarmlandBlock;

impl BlockBehaviour for FarmlandBlock {
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        // TODO: 向上推动实体
        args.world.set_block_state(
            args.position,
            Block::DIRT.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            return Block::DIRT.default_state.id;
        }
        args.block.default_state.id
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction == BlockDirection::Up && !can_place_at(args.world, args.position) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        }
        args.state_id
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        // TODO: 添加降雨检查。记得确认哪种方式性能最优。
        if is_water_nearby(args.world, args.position) {
            let mut props = FarmlandProperties::default(args.block);
            let mut new_moisture = 7;
            if let Some(server) = args.world.server.upgrade() {
                let mut event =
                    crate::plugin::api::events::block::moisture_change::MoistureChangeEvent::new(
                        *args.position,
                        args.world.clone(),
                        new_moisture,
                    );
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return;
                }
                new_moisture = event.new_moisture;
            }
            props.moisture = new_moisture.clamp(0, 7) as u8;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_NEIGHBORS,
            );
        } else {
            let state_id = args.world.get_block_state_id(args.position);
            let mut props = FarmlandProperties::from_state_id(state_id);
            if props.moisture == 0 {
                if !args
                    .world
                    .get_block(&args.position.up())
                    .has_tag(&tag::Block::MINECRAFT_MAINTAINS_FARMLAND)
                {
                    //TODO 将实体向上推
                    args.world.set_block_state(
                        args.position,
                        Block::DIRT.default_state.id,
                        BlockFlags::NOTIFY_NEIGHBORS,
                    );
                }
            } else {
                let mut new_moisture = (props.moisture as i32 - 1).clamp(0, 7);
                if let Some(server) = args.world.server.upgrade() {
                    let mut event = crate::plugin::api::events::block::moisture_change::MoistureChangeEvent::new(
                        *args.position,
                        args.world.clone(),
                        new_moisture,
                    );
                    server.plugin_manager.fire_blocking(&server, &mut event);
                    if event.cancelled {
                        return;
                    }
                    new_moisture = event.new_moisture;
                }
                props.moisture = new_moisture.clamp(0, 7) as u8;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_NEIGHBORS,
                );
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
    let state = world.get_block_state(&block_pos.up());
    !state.is_solid() // TODO: 添加栅栏门方块
}

fn is_water_nearby(world: &Arc<World>, block_pos: &BlockPos) -> bool {
    for dx in -4..=4 {
        for dy in 0..=1 {
            for dz in -4..=4 {
                let check_pos = block_pos.offset(Vector3 {
                    x: dx,
                    y: dy,
                    z: dz,
                });
                //TODO 这里应使用 water 标签，但它目前似乎不起作用。
                if world.get_block(&check_pos) == &Block::WATER {
                    return true;
                }
            }
        }
    }
    false
}
