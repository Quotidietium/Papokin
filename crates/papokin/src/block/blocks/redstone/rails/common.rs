use std::sync::Arc;

use papokin_data::{
    Block, BlockDirection, BlockStateId,
    block_properties::{HorizontalFacing, RailShape, RailShapeStraight},
};
use papokin_util::math::position::BlockPos;
use papokin_world::world::{BlockAccessor, BlockFlags};

use crate::world::World;

use super::{HorizontalFacingRailExt, Rail, RailElevation, RailProperties, StraightRailShapeExt};

pub(super) fn rail_placement_is_valid(world: &World, block: &Block, pos: &BlockPos) -> bool {
    if !can_place_rail_at(world, pos) {
        return false;
    }

    let state_id = world.get_block_state_id(pos);
    let rail_props = RailProperties::new(state_id, block);
    let rail_leaning_direction = match rail_props.shape() {
        RailShape::AscendingNorth => Some(HorizontalFacing::North),
        RailShape::AscendingSouth => Some(HorizontalFacing::South),
        RailShape::AscendingEast => Some(HorizontalFacing::East),
        RailShape::AscendingWest => Some(HorizontalFacing::West),
        _ => None,
    };

    if let Some(direction) = rail_leaning_direction
        && !can_place_rail_at(world, &pos.offset(direction.to_offset()).up())
    {
        return false;
    }

    true
}

pub(super) fn can_place_rail_at(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let state = world.get_block_state(&pos.down());
    state.is_side_solid(BlockDirection::Up)
}

pub(super) fn compute_placed_rail_shape(
    world: &World,
    block_pos: &BlockPos,
    player_facing: HorizontalFacing,
) -> RailShapeStraight {
    // 使用与普通铁轨相同的精密逻辑，但为直线铁轨做了适配
    // 类似普通铁轨放置，检查各方向的铁轨连接

    // 先检查东方
    if let Some(east_rail) = Rail::find_if_unlocked(world, block_pos, HorizontalFacing::East) {
        // 检查对侧连接（西）以形成直线
        if let Some(west_rail) = Rail::find_if_unlocked(world, block_pos, HorizontalFacing::West) {
            // 东面和西面都有连接
            if east_rail.elevation == RailElevation::Up {
                return RailShapeStraight::AscendingEast;
            } else if west_rail.elevation == RailElevation::Up {
                return RailShapeStraight::AscendingWest;
            }
            return RailShapeStraight::EastWest;
        }
        // 仅东侧连接
        if east_rail.elevation == RailElevation::Up {
            return RailShapeStraight::AscendingEast;
        }
        return RailShapeStraight::EastWest;
    }

    // 检查南方
    if let Some(south_rail) = Rail::find_if_unlocked(world, block_pos, HorizontalFacing::South) {
        // 检查对侧连接（北）以形成直线
        if let Some(north_rail) = Rail::find_if_unlocked(world, block_pos, HorizontalFacing::North)
        {
            // 南面和北面都有连接
            if south_rail.elevation == RailElevation::Up {
                return RailShapeStraight::AscendingSouth;
            } else if north_rail.elevation == RailElevation::Up {
                return RailShapeStraight::AscendingNorth;
            }
            return RailShapeStraight::NorthSouth;
        }
        // 仅南侧连接
        if south_rail.elevation == RailElevation::Up {
            return RailShapeStraight::AscendingSouth;
        }
        return RailShapeStraight::NorthSouth;
    }

    // 检查西方
    if let Some(west_rail) = Rail::find_if_unlocked(world, block_pos, HorizontalFacing::West) {
        if west_rail.elevation == RailElevation::Up {
            return RailShapeStraight::AscendingWest;
        }
        return RailShapeStraight::EastWest;
    }

    // 检查北方
    if let Some(north_rail) = Rail::find_if_unlocked(world, block_pos, HorizontalFacing::North) {
        if north_rail.elevation == RailElevation::Up {
            return RailShapeStraight::AscendingNorth;
        }
        return RailShapeStraight::NorthSouth;
    }

    // 未找到连接，使用玩家朝向
    player_facing.to_rail_shape_flat()
}

pub(super) fn update_flanking_rails_shape(
    world: &Arc<World>,
    block: &Block,
    state_id: BlockStateId,
    block_pos: &BlockPos,
) {
    for direction in RailProperties::new(state_id, block).directions() {
        let Some(mut flanking_rail) =
            Rail::find_with_elevation(world, block_pos.offset(direction.to_offset()))
        else {
            // 跳过非铁轨方块
            continue;
        };

        let new_shape =
            compute_flanking_rail_new_shape(world, &flanking_rail, direction.opposite());

        if new_shape != flanking_rail.properties.shape() {
            flanking_rail.properties.set_shape(new_shape);
            world.set_block_state(
                &flanking_rail.position,
                flanking_rail.properties.to_state_id(flanking_rail.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}

fn compute_flanking_rail_new_shape(
    world: &World,
    rail: &Rail,
    flanking_from: HorizontalFacing,
) -> RailShape {
    let mut connected_towards = Vec::with_capacity(2);
    let mut is_already_connected_to_elevated_rail = false;

    for neighbor_direction in rail.properties.directions() {
        if neighbor_direction == flanking_from {
            // 指向玩家放置位置的铁轨未连接
            continue;
        }

        let Some(maybe_connected_rail) =
            Rail::find_with_elevation(world, rail.position.offset(neighbor_direction.to_offset()))
        else {
            // 指向非铁轨方块的铁轨未连接
            continue;
        };

        if maybe_connected_rail
            .properties
            .directions()
            .into_iter()
            .any(|d| d == neighbor_direction.opposite())
        {
            // 指向回指的其他铁轨的铁轨已连接
            connected_towards.push(neighbor_direction);
            is_already_connected_to_elevated_rail =
                maybe_connected_rail.elevation == RailElevation::Up;
        }
    }

    let new_neighbor_directions = match connected_towards.len() {
        2 => {
            // 不要更新已锁定的铁轨（即完全连接的）
            return rail.properties.shape();
        }
        1 => [connected_towards[0], flanking_from],
        0 => [flanking_from, flanking_from.opposite()],
        _ => {
            tracing::error!("铁轨只有两个方向，但得到了 {}", connected_towards.len());
            return rail.properties.shape();
        }
    };

    // 处理想要保持笔直的铁轨
    if new_neighbor_directions
        .iter()
        .all(|d| *d == flanking_from || *d == flanking_from.opposite())
    {
        if rail.elevation == RailElevation::Down {
            if is_already_connected_to_elevated_rail {
                // 按南/西方向升序优先
                return match flanking_from {
                    HorizontalFacing::South | HorizontalFacing::North => RailShape::AscendingSouth,
                    HorizontalFacing::West | HorizontalFacing::East => RailShape::AscendingWest,
                };
            }

            return flanking_from.to_rail_shape_ascending_towards().as_shape();
        } else if is_already_connected_to_elevated_rail {
            return connected_towards[0]
                .to_rail_shape_ascending_towards()
                .as_shape();
        }

        // 即使铁轨已有良好方向，也将形状重置为平直
        return rail.get_new_rail_shape(new_neighbor_directions[0], new_neighbor_directions[1]);
    }

    // 处理想要弯曲的直铁轨
    if !rail.properties.can_curve() {
        return if new_neighbor_directions[0] == HorizontalFacing::North
            || new_neighbor_directions[0] == HorizontalFacing::South
        {
            if rail.elevation == RailElevation::Down {
                // 铁轨朝下，所以它应该正在上行
                flanking_from.to_rail_shape_ascending_towards().as_shape()
            } else {
                rail.get_new_rail_shape(new_neighbor_directions[0], new_neighbor_directions[1])
            }
        } else {
            rail.properties.shape()
        };
    }

    rail.get_new_rail_shape(new_neighbor_directions[0], new_neighbor_directions[1])
}
