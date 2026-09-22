use papokin_data::tag::Taggable;
use papokin_data::{
    Block, BlockState, BlockStateId, block_properties::SnowLikeProperties, item::Item, tag,
};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs,
    PathComputationType, RandomTickArgs, UseWithItemArgs, registry::BlockActionResult,
};

#[pumpkin_block("minecraft:snow")]
pub struct LayeredSnowBlock;

impl BlockBehaviour for LayeredSnowBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            return Block::AIR.default_state.id;
        }
        let mut props = SnowLikeProperties::default(args.block);
        props.layers = 1;
        props.to_state_id(&Block::SNOW)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        {
            let item = args.item_stack.item;

            if item == &Item::SNOW {
                let pos = if args.hit.face.is_horizontal() {
                    &args.position.offset(args.hit.face.to_offset())
                } else {
                    args.position
                };
                if !can_place_at(args.world.as_ref(), pos) {
                    return BlockActionResult::Pass;
                }
                let (block, state_id) = args.world.get_block_and_state_id(pos);

                if block != &Block::SNOW {
                    return BlockActionResult::Pass;
                }

                let mut props = SnowLikeProperties::from_state_id(state_id);
                if props.layers >= 8 {
                    args.world.set_block_state(
                        pos,
                        Block::SNOW_BLOCK.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                    return BlockActionResult::Success;
                }
                props.layers += 1;

                let state_id = props.to_state_id(&Block::SNOW);
                args.world
                    .set_block_state(pos, state_id, BlockFlags::NOTIFY_ALL);
                return BlockActionResult::Success;
            }
            BlockActionResult::Pass
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_place_at(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::empty());
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        // 雪层在亮度高于 11 级的方块光照下会融化，
        // 例如来自附近的火把。
        if args.world.get_block_light_level(args.position).unwrap_or(0) > 11 {
            let mut event = crate::plugin::api::events::block::block_fade::BlockFadeEvent::new(
                *args.position,
                args.world.get_block(args.position),
            );
            if let Some(server) = args.world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return;
            }

            args.world
                .break_block(args.position, None, BlockFlags::empty());
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        }
        args.state_id
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        computation_type == PathComputationType::Land
            && SnowLikeProperties::from_state_id(state.id).layers < 5
    }
}

fn can_place_at(block_accessor: &dyn BlockAccessor, position: &BlockPos) -> bool {
    let below_pos = position.down();
    let (below_block, state) = block_accessor.get_block_and_state(&below_pos);

    if below_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_SNOW_LAYER) {
        return false;
    }
    if below_block.has_tag(&tag::Block::MINECRAFT_SUPPORT_OVERRIDE_SNOW_LAYER) {
        return true;
    }

    // Block.isFaceFullSquare(collisionShape, Direction.UP)：碰撞形状必须完全覆盖
    // 顶面，例如树叶并非"侧面实心"，但确实能支撑雪层。
    state.get_block_collision_shapes().any(|shape| {
        shape.max.y >= 1.0
            && shape.min.x <= 0.0
            && shape.max.x >= 1.0
            && shape.min.z <= 0.0
            && shape.max.z >= 1.0
    }) || (below_block == &Block::SNOW && SnowLikeProperties::from_state_id(state.id).layers == 8)
}
