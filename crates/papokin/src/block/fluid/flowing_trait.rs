use super::{pathfinder, physics};
use crate::world::World;
use papokin_data::{
    Block, BlockDirection, BlockStateId,
    fluid::{EnumVariants, Falling, Fluid, FluidProperties, Level},
};
use papokin_util::math::position::BlockPos;
use papokin_world::{tick::TickPriority, world::BlockFlags};
use std::sync::Arc;
pub type FlowingFluidProperties = papokin_data::fluid::FlowingWaterLikeFluidProperties;

#[allow(async_fn_in_trait)]
pub trait FlowingFluid: Send + Sync {
    fn get_level_decrease_per_block(&self, world: &World) -> i32;
    fn get_flow_speed(&self, world: &World) -> u8;

    fn get_source(&self, fluid: &Fluid, falling: bool) -> FlowingFluidProperties {
        let mut source_props = FlowingFluidProperties::default(fluid);
        source_props.level = Level::L8;
        source_props.falling = if falling {
            Falling::True
        } else {
            Falling::False
        };
        source_props
    }

    fn get_flowing(&self, fluid: &Fluid, level: Level, falling: bool) -> FlowingFluidProperties {
        let mut flowing_props = FlowingFluidProperties::default(fluid);
        flowing_props.level = level;
        flowing_props.falling = if falling {
            Falling::True
        } else {
            Falling::False
        };
        flowing_props
    }

    fn get_max_flow_distance(&self, world: &World) -> i32;
    fn can_convert_to_source(&self, world: &Arc<World>) -> bool;

    /// 若 `state_id` 表示给定的流体，则返回 true——既可以是直接的流体状态
    /// 或作为含水方块（当流体为水类型时）。
    fn has_fluid_at(&self, fluid: &Fluid, state_id: BlockStateId) -> bool {
        self.get_effective_props(fluid, state_id).is_some()
    }

    ///为给定状态返回正确的流体属性，将含水方块视为水源
    /// (等级 8，非下落状态)。如果该状态不包含此流体，则返回 `None`。
    fn get_effective_props(
        &self,
        fluid: &Fluid,
        state_id: BlockStateId,
    ) -> Option<FlowingFluidProperties> {
        if fluid.states.iter().any(|s| s.block_state_id == state_id) {
            return Some(FlowingFluidProperties::from_state_id(state_id, fluid));
        }
        if fluid.id == Fluid::FLOWING_WATER.id || fluid.id == Fluid::WATER.id {
            let block = Block::from_state_id(state_id);
            if block.is_waterlogged(state_id) {
                return Some(self.get_source(fluid, false));
            }
        }
        None
    }

    /// 核心流体刻处理程序，更新流体状态并触发扩散。
    ///
    /// 通过以下方式处理已调度的流体刻：
    /// 1. 校验方块中含有流体
    /// 2. 根据邻居方块更新非源流体的等级
    /// 3. 触发流体向相邻位置扩散
    ///
    /// 水源（等级 8，非下落状态）总是直接扩散而不改变方块状态。
    fn on_scheduled_tick_internal(&self, world: &Arc<World>, fluid: &Fluid, block_pos: &BlockPos) {
        let current_block_state_id = world.get_block_state_id(block_pos);
        let block = Block::from_state_id(current_block_state_id);

        if !self.has_fluid_at(fluid, current_block_state_id) {
            return;
        }

        let waterlogged = block.is_waterlogged(current_block_state_id);
        let Some(current_fluid_state) = self.get_effective_props(fluid, current_block_state_id)
        else {
            return;
        };
        let is_source =
            current_fluid_state.level == Level::L8 && current_fluid_state.falling != Falling::True;
        let state_for_spreading: FlowingFluidProperties;

        // 若非源方块则更新状态
        if !is_source && !waterlogged {
            let new_fluid_state = self.get_new_liquid(world, fluid, block_pos);

            if let Some(new_state) = new_fluid_state {
                let new_state_id = new_state.to_state_id(fluid);

                if new_state_id != current_block_state_id {
                    world.set_block_state(block_pos, new_state_id, BlockFlags::NOTIFY_ALL);

                    // 为该位置调度下一刻
                    let tick_delay = self.get_flow_speed(world);
                    world.schedule_fluid_tick(fluid, *block_pos, tick_delay, TickPriority::Normal);
                }

                // 使用新状态进行传播
                state_for_spreading = new_state;
            } else {
                if !waterlogged {
                    world.set_block_state(
                        block_pos,
                        Block::AIR.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                }
                return; // 流体消失后不再扩散
            }
        } else {
            // 源方块使用其当前状态
            state_for_spreading = current_fluid_state;
        }

        // 然后使用合适的状态进行蔓延
        self.try_flow(world, fluid, block_pos, &state_for_spreading);
    }

    /// 尝试让流体从某个位置流动，优先向下流动。
    ///
    /// 流动优先级：
    /// 1. 下方 - 如果下方有空间，创建下落流体（等级 8）
    /// 2. 侧面 - 通过寻路水平扩散
    ///
    /// 当存在 3 个及以上相邻水源时，向下流动也会向侧面扩散。
    fn try_flow(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        block_pos: &BlockPos,
        props: &FlowingFluidProperties,
    ) {
        let below_pos = block_pos.down();
        let below_state = world.get_block_state(&below_pos);
        let below_block = Block::from_state_id(below_state.id);
        let is_hole = physics::can_be_replaced(below_state, below_block, fluid);

        // 先尝试向下流动
        if is_hole {
            let falling_props = self.get_flowing(fluid, Level::L8, true);
            self.spread_to(world, fluid, &below_pos, falling_props.to_state_id(fluid));

            // 检查是否也应向侧面蔓延
            if props.level == Level::L8 && props.falling == Falling::False {
                let source_count = self.count_source_neighbors(world, fluid, block_pos);
                if source_count >= 3 {
                    self.flow_to_sides(world, fluid, block_pos, props);
                }
            }
            return;
        }

        // 检查流体是否应向侧面流动
        self.flow_to_sides(world, fluid, block_pos, props);
    }

    fn count_source_neighbors(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        block_pos: &BlockPos,
    ) -> i32 {
        let mut count = 0;
        for direction in [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::West,
            BlockDirection::East,
        ] {
            let neighbor_pos = block_pos.offset(direction.to_offset());
            let neighbor_id = world.get_block_state_id(&neighbor_pos);
            if self
                .get_effective_props(fluid, neighbor_id)
                .is_some_and(|p| p.level == Level::L8 && p.falling == Falling::False)
            {
                count += 1;
            }
        }
        count
    }

    /// 根据相邻方块与环境计算某位置的新流体状态。
    ///
    /// 优先顺序：
    /// 1. 源方块保持不变
    /// 2. 无限源的形成（2 个及以上相邻源 + 下方为固体方块或源）
    /// 3. 上方有流体时强制进入下落状态（等级 8，下落）
    /// 4. 标准流动计算：最高邻居减去落差
    ///
    /// # Returns
    /// 新的流体属性；若流体应流失则为 None
    fn get_new_liquid(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        block_pos: &BlockPos,
    ) -> Option<FlowingFluidProperties> {
        let current_state_id = world.get_block_state_id(block_pos);
        let current_props = FlowingFluidProperties::from_state_id(current_state_id, fluid);

        // 源方块永不变化
        if current_props.level == Level::L8 && current_props.falling != Falling::True {
            return Some(current_props);
        }

        // 首先：检查水平邻居是否可形成无限水源
        let mut highest_neighbor = 0;
        let mut neighbor_source_count = 0;
        for direction in [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::West,
            BlockDirection::East,
        ] {
            let neighbor_pos = block_pos.offset(direction.to_offset());
            let neighbor_state_id = world.get_block_state_id(&neighbor_pos);
            let Some(neighbor_props) = self.get_effective_props(fluid, neighbor_state_id) else {
                continue;
            };

            // 统计水平的非下坠源，用于形成无限源
            if neighbor_props.level == Level::L8 && neighbor_props.falling == Falling::False {
                neighbor_source_count += 1;
            }

            // 从侧面流入的水计为 8 级
            let neighbor_level = if neighbor_props.falling == Falling::True {
                8
            } else {
                i32::from(neighbor_props.level.to_index()) + 1
            };

            highest_neighbor = highest_neighbor.max(neighbor_level);
        }

        // 先尝试形成无限水源
        if self.can_convert_to_source(world) && neighbor_source_count >= 2 {
            let below_pos = block_pos.down();
            let below_state = world.get_block_state(&below_pos);
            let below_state_id = below_state.id;

            // 检查下方方块是否为同种流体的稳定源
            let below_is_same_source = self
                .get_effective_props(fluid, below_state_id)
                .is_some_and(|p| p.level == Level::L8 && p.falling == Falling::False);

            // 如果下方是实心方块或同种流体的源，则在此处生成源。
            if below_is_same_source || below_state.is_solid_block() {
                return Some(self.get_source(fluid, false));
            }
            // 否则继续执行标准的下落/流动逻辑
        }

        // 然后：如果上方有水，此方块总是为 level 8 且 falling=true
        let above_pos = block_pos.up();
        let above_state_id = world.get_block_state_id(&above_pos);

        if self.has_fluid_at(fluid, above_state_id) {
            return Some(self.get_flowing(fluid, Level::L8, true));
        }

        // 标准的流动计算
        let drop_off = self.get_level_decrease_per_block(world);
        let new_level = highest_neighbor - drop_off;

        if new_level <= 0 {
            None
        } else {
            Some(self.get_flowing(fluid, Level::from_index(new_level as u16 - 1), false))
        }
    }

    /// 包含静止检查与状态更新的核心扩散逻辑。
    ///
    /// 实现了：
    /// - 静止状态：防止不必要的更新（例如水源方块、较低水位）
    /// - 无限源形成检查（放置前后）
    /// - 对非流体方块进行方块替换
    /// - 为非源方块调度流体刻
    ///
    /// 由 `spread_to` 的实现在完成流体相关预检查后调用。
    fn apply_spread(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        pos: &BlockPos,
        state_id: BlockStateId,
        new_props: FlowingFluidProperties,
    ) {
        let current_state_id = world.get_block_state_id(pos);
        if let Some(current_props) = self.get_effective_props(fluid, current_state_id) {
            let current_level = i32::from(current_props.level.to_index()) + 1;
            let new_level = i32::from(new_props.level.to_index()) + 1;
            let current_is_source =
                current_props.level == Level::L8 && current_props.falling == Falling::False;
            let new_is_source = new_props.level == Level::L8 && new_props.falling == Falling::False;

            // 永不用任何东西覆盖源
            if current_is_source {
                return;
            }

            // 在静止检查之前检查是否形成无限水源
            if !current_is_source && self.can_convert_to_source(world) {
                let should_convert = self.check_infinite_source_formation(world, fluid, pos);

                if should_convert {
                    let source_props = self.get_source(fluid, false);
                    let source_state_id = source_props.to_state_id(fluid);
                    world.set_block_state(pos, source_state_id, BlockFlags::NOTIFY_ALL);

                    // 源方块不需要刻更新
                    return;
                }
            }

            // 如果 new 是源，则始终接受它
            if new_is_source {
                // 继续在下方设置状态
            } else if current_props.falling == new_props.falling {
                // 相同的下落状态 - 检查等级
                if new_level <= current_level {
                    return;
                }
            }
        } else {
            // 替换非流体方块
            let block = world.get_block(pos);
            if block.id != Block::AIR.id {
                world.break_block(pos, None, BlockFlags::NOTIFY_ALL);
            }
        }

        let mut event = crate::plugin::api::events::block::block_from_to::BlockFromToEvent::new(
            *pos,
            *pos,
            &papokin_data::Block::WATER,
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        let mut level_event =
            crate::plugin::api::events::block::fluid_level_change::FluidLevelChangeEvent::new(
                *pos,
                world.clone(),
                state_id,
            );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut level_event);
        }
        if level_event.cancelled {
            return;
        }

        world.set_block_state(pos, state_id, BlockFlags::NOTIFY_ALL);

        // 在放置新流体后检查是否形成无限水源
        if self.can_convert_to_source(world) {
            let should_convert = self.check_infinite_source_formation(world, fluid, pos);

            if should_convert {
                let source_props = self.get_source(fluid, false);
                let source_state_id = source_props.to_state_id(fluid);
                world.set_block_state(pos, source_state_id, BlockFlags::NOTIFY_ALL);

                // 源方块不需要刻更新
                return;
            }
        }

        // 仅在不是水源时才调度刻
        let is_source = new_props.level == Level::L8 && new_props.falling == Falling::False;

        if !is_source {
            let tick_delay = self.get_flow_speed(world);
            world.schedule_fluid_tick(fluid, *pos, tick_delay, TickPriority::Normal);
        }
    }

    /// 检查是否满足无限流体源的形成条件。
    ///
    /// 要求：
    /// - 2 个及以上水平相邻的源方块（等级 8，非下落）
    /// - 下方方块是实心方块，或者是同一流体的源方块
    ///
    /// # Returns
    /// 如果该位置应转换为源方块则为 `true`
    fn check_infinite_source_formation(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        pos: &BlockPos,
    ) -> bool {
        // 统计相邻的水平源方块
        let mut source_count = 0;
        for direction in [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::West,
            BlockDirection::East,
        ] {
            let neighbor_pos = pos.offset(direction.to_offset());
            let neighbor_state_id = world.get_block_state_id(&neighbor_pos);

            if self
                .get_effective_props(fluid, neighbor_state_id)
                .is_some_and(|p| p.level == Level::L8 && p.falling == Falling::False)
            {
                source_count += 1;
            }
        }

        // 至少需要 2 个源邻居
        if source_count < 2 {
            return false;
        }

        // 检查下方方块
        let below_pos = pos.down();
        let below_state = world.get_block_state(&below_pos);
        let below_state_id = below_state.id;

        // 检查下方方块是否为同种流体的稳定源
        let below_is_same_source = self
            .get_effective_props(fluid, below_state_id)
            .is_some_and(|p| p.level == Level::L8 && p.falling == Falling::False);

        // 若下方为固体或同类流体源，则转换为源
        below_is_same_source || below_state.is_solid_block()
    }

    /// 以给定状态将流体传播到目标位置。
    ///
    /// 默认实现委托给 `apply_spread`。诸如……的实现
    /// 熔岩可覆盖此方法以添加特定流体的逻辑（如水 -> 石头的转化）。
    fn spread_to(&self, world: &Arc<World>, fluid: &Fluid, pos: &BlockPos, state_id: BlockStateId) {
        let new_props = FlowingFluidProperties::from_state_id(state_id, fluid);
        self.apply_spread(world, fluid, pos, state_id, new_props);
    }

    /// 使用寻路将流体水平传播到相邻位置。
    ///
    /// 使用 `get_spread` 寻找最优流向（到洞的最短距离）
    /// 以及为每个目标位置计算出的流体状态。
    fn flow_to_sides(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        block_pos: &BlockPos,
        props: &FlowingFluidProperties,
    ) {
        let drop_off = self.get_level_decrease_per_block(world);
        let current_level = i32::from(props.level.to_index()) + 1;
        let effective_level = if props.falling == Falling::True {
            7
        } else {
            current_level - drop_off
        };

        if effective_level <= 0 {
            return;
        }

        let (spread_dirs, count) = pathfinder::get_spread(self, world, fluid, block_pos);

        for &(direction, state_id) in spread_dirs.iter().take(count) {
            let side_pos = block_pos.offset(direction.to_offset());

            self.spread_to(world, fluid, &side_pos, state_id);
        }
    }
}
