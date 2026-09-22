use super::physics;
use crate::block::fluid::flowing_trait::FlowingFluid;
use crate::world::World;
use papokin_data::BlockStateId;
use papokin_data::{
    Block, BlockDirection,
    fluid::{EnumVariants, Falling, Fluid, FluidProperties, Level},
};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 表示流体流动计算中 BFS 寻路队列里的节点。
#[derive(Clone, Copy)]
pub struct PathNode {
    pub pos: BlockPos,
    pub distance: i32,
    pub exclude_dir: BlockDirection,
}

/// 检查某位置下方是否存在可向下流动的空隙。
fn is_hole(world: &Arc<World>, fluid: &Fluid, pos: &BlockPos) -> bool {
    let below_pos = pos.down();
    let below_state = world.get_block_state(&below_pos);
    let below_block = Block::from_state_id(below_state.id);
    physics::can_be_replaced(below_state, below_block, fluid)
}

/// 使用洞优先策略确定流体的有效扩散方向。
///
/// - 洞（可向下流动的机会）获得距离 0 的优先级
/// - 返回所有具有相同最小距离的方向
/// - 返回最多 4 个方向及其计算出的流体状态
pub fn get_spread<T: FlowingFluid + Sync + ?Sized>(
    fluid_impl: &T,
    world: &Arc<World>,
    fluid: &Fluid,
    block_pos: &BlockPos,
) -> ([(BlockDirection, BlockStateId); 4], usize) {
    let mut min_dist = 1000;
    let mut result = [(BlockDirection::North, BlockStateId::default()); 4];
    let mut result_count = 0;

    for direction in [
        BlockDirection::North,
        BlockDirection::South,
        BlockDirection::West,
        BlockDirection::East,
    ] {
        let side_pos = block_pos.offset(direction.to_offset());
        let side_state = world.get_block_state(&side_pos);
        let side_state_id = side_state.id;
        let side_block = Block::from_state_id(side_state.id);

        let side_fluid_props = fluid_impl.get_effective_props(fluid, side_state_id);

        // 检查我们能否穿过（不是实心源方块或含水方块）
        if !physics::can_be_replaced(side_state, side_block, fluid)
            || side_fluid_props
                .as_ref()
                .is_some_and(|p| p.level == Level::L8 && p.falling != Falling::True)
        {
            continue;
        }

        // 若该位置没有有效的流体状态则跳过
        let Some(new_fluid_props) = fluid_impl.get_new_liquid(world, fluid, &side_pos) else {
            continue;
        };

        let new_state_id = new_fluid_props.to_state_id(fluid);

        // 孔洞的距离为 0
        let slope_dist = if is_hole(world, fluid, &side_pos) {
            0
        } else {
            get_in_flow_down_distance_iterative(
                fluid_impl,
                world,
                fluid,
                side_pos,
                direction.opposite(),
            )
        };

        // 若找到更短路径则清除结果
        if slope_dist < min_dist {
            result_count = 0;
        }

        // 添加所有最小距离相等的方向
        if slope_dist <= min_dist {
            // 检查该位置的流体是否可被替换
            let can_replace = side_fluid_props.as_ref().is_none_or(|sp| {
                // 若新等级更高或目标正在下落则可替换
                let target_level = i32::from(sp.level.to_index()) + 1;
                let new_level = i32::from(new_fluid_props.level.to_index()) + 1;
                new_level > target_level || sp.falling == Falling::True
            });

            if can_replace && result_count < 4 {
                result[result_count] = (direction, new_state_id);
                result_count += 1;
            }

            min_dist = slope_dist;
        }
    }
    (result, result_count)
}

/// 执行迭代式 BFS 搜索，以找到达向下流动机会的最短距离。
///
/// 使用栈分配数组以实现零堆分配。最多搜索 `get_max_flow_distance`
/// (动态) 从起始位置水平扩散。
///
/// # Returns
/// 到最近洞的距离；若在搜索距离内未找到洞，则为 1000
pub fn get_in_flow_down_distance_iterative<T: FlowingFluid + Sync + ?Sized>(
    fluid_impl: &T,
    world: &Arc<World>,
    fluid: &Fluid,
    start_pos: BlockPos,
    initial_exclude_dir: BlockDirection,
) -> i32 {
    const MAX_QUEUE_SIZE: usize = 64;

    let mut queue: [PathNode; MAX_QUEUE_SIZE] = [PathNode {
        pos: BlockPos::new(0, 0, 0),
        distance: 0,
        exclude_dir: BlockDirection::North,
    }; MAX_QUEUE_SIZE];

    let mut queue_start = 0;
    let mut queue_end = 0;

    queue[queue_end] = PathNode {
        pos: start_pos,
        distance: 1,
        exclude_dir: initial_exclude_dir,
    };
    queue_end = 1;

    let mut visited_bitset = [0u64; 4];
    let slope_find_distance = fluid_impl.get_max_flow_distance(world);

    let get_bit_index = |pos: BlockPos| -> Option<usize> {
        let dx = pos.0.x - start_pos.0.x + slope_find_distance;
        let dz = pos.0.z - start_pos.0.z + slope_find_distance;
        let grid_size = slope_find_distance * 2 + 1;
        (dx >= 0 && dx < grid_size && dz >= 0 && dz < grid_size)
            .then(|| (dz * grid_size + dx) as usize)
    };

    while queue_start < queue_end {
        let node = queue[queue_start];
        queue_start += 1;

        if node.distance > slope_find_distance {
            continue;
        }

        if let Some(bit_idx) = get_bit_index(node.pos) {
            let word_idx = bit_idx / 64;
            let bit_pos = bit_idx % 64;
            if (visited_bitset[word_idx] & (1u64 << bit_pos)) != 0 {
                continue;
            }
            visited_bitset[word_idx] |= 1u64 << bit_pos;
        }

        // 检查洞口（向下流动的机会）
        let below_pos = node.pos.down();
        let below_state = world.get_block_state(&below_pos);
        let below_block = Block::from_state_id(below_state.id);
        if physics::can_be_replaced(below_state, below_block, fluid) {
            return node.distance;
        }

        for direction in [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::West,
            BlockDirection::East,
        ] {
            if direction == node.exclude_dir {
                continue;
            }

            let next_pos = node.pos.offset(direction.to_offset());

            let next_state = world.get_block_state(&next_pos);
            let next_block = Block::from_state_id(next_state.id);
            if !physics::can_be_replaced(next_state, next_block, fluid) {
                continue;
            }

            // 源方块（包括含水的）阻挡水平寻路
            let next_state_id = world.get_block_state_id(&next_pos);
            if fluid_impl
                .get_effective_props(fluid, next_state_id)
                .is_some_and(|p| p.level == Level::L8 && p.falling == Falling::False)
            {
                continue;
            }

            if queue_end < MAX_QUEUE_SIZE {
                queue[queue_end] = PathNode {
                    pos: next_pos,
                    distance: node.distance + 1,
                    exclude_dir: direction.opposite(),
                };
                queue_end += 1;
            }
        }
    }

    1000
}
