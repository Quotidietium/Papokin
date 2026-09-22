use crate::chunk::io::Dirtiable;
use crate::chunk::palette::BlockPalette;
use crate::level::Level;
use crossbeam::queue::SegQueue;
use papokin_config::lighting::LightingEngineConfig;
use papokin_data::BlockDirection;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

pub struct DynamicLightEngine {
    min_y: i32,
    max_y: i32,
    block_decrease: SegQueue<(BlockPos, u8)>,
    block_increase: SegQueue<(BlockPos, u8)>,
    sky_decrease: SegQueue<(BlockPos, u8)>,
    sky_increase: SegQueue<(BlockPos, u8)>,
}

impl DynamicLightEngine {
    #[must_use]
    pub const fn new(min_y: i32, max_y: i32) -> Self {
        Self {
            min_y,
            max_y,
            block_decrease: SegQueue::new(),
            block_increase: SegQueue::new(),
            sky_decrease: SegQueue::new(),
            sky_increase: SegQueue::new(),
        }
    }
}

impl DynamicLightEngine {
    /// 检查给定位置上方是否为开阔天空（无不透明方块阻挡天空光）。
    fn has_open_sky_above(&self, level: &Arc<Level>, pos: &BlockPos) -> bool {
        let max_y = self.max_y - 1;
        let mut current_pos = *pos;

        // 向上扫描直到碰到天空或是不透明方块
        while current_pos.0.y < max_y {
            current_pos.0.y += 1;

            let state = level.get_block_state(&current_pos).to_state();
            if state.can_occlude() || state.opacity > 0 {
                return false; // 在到达天空前命中不透明或遮光方块
            }
        }

        true // 到达天空且未碰到不透明方块
    }

    /// 处理由方块变更（放置/破坏）触发的所有光照更新。
    /// 此方法更新方块光照和天空光照，并确保光源方块有效。
    pub fn update_lighting_at(&self, level: &Arc<Level>, pos: BlockPos) {
        // 方块光照
        self.check_block_light_updates(level, pos);
        self.perform_block_light_updates(level);

        // 天空光照
        self.check_sky_light_updates(level, pos);
        self.perform_sky_light_updates(level);
    }

    pub fn queue_block_light_decrease(&self, pos: BlockPos, level: u8) {
        self.block_decrease.push((pos, level));
    }

    pub fn queue_block_light_increase(&self, pos: BlockPos, level: u8) {
        self.block_increase.push((pos, level));
    }

    pub fn queue_sky_light_decrease(&self, pos: BlockPos, level: u8) {
        self.sky_decrease.push((pos, level));
    }

    pub fn queue_sky_light_increase(&self, pos: BlockPos, level: u8) {
        self.sky_increase.push((pos, level));
    }

    pub fn perform_block_light_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;

        // 持续处理直到两个队列都为空
        // 光照传播会排队新的更新，因此需要处理至收敛
        loop {
            let decrease_updates = self.perform_block_light_decrease_updates(level);
            let increase_updates = self.perform_block_light_increase_updates(level);

            updates += decrease_updates + increase_updates;

            // 没有更多更新被处理时停止
            if decrease_updates == 0 && increase_updates == 0 {
                break;
            }
        }

        updates
    }

    fn perform_block_light_decrease_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;

        while let Some((pos, expected_light)) = self.block_decrease.pop() {
            self.propagate_block_light_decrease(level, &pos, expected_light);
            updates += 1;
        }

        updates
    }

    fn perform_block_light_increase_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;

        while let Some((pos, expected_light)) = self.block_increase.pop() {
            self.propagate_block_light_increase(level, &pos, expected_light);
            updates += 1;
        }

        updates
    }

    fn propagate_block_light_increase(&self, level: &Arc<Level>, pos: &BlockPos, light_level: u8) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }

            if let Some(neighbor_light) = self.get_block_light_level(level, &neighbor_pos) {
                let neighbor_state = level.get_block_state(&neighbor_pos).to_state();
                let opacity = neighbor_state.opacity.max(1);
                let new_light = light_level.saturating_sub(opacity);

                // 仅当新光比当前光更亮时才传播
                if new_light > neighbor_light
                    && self
                        .set_block_light_level(level, &neighbor_pos, new_light)
                        .is_ok()
                    && new_light > 1
                {
                    self.queue_block_light_increase(neighbor_pos, new_light);
                }
            }
        }
    }

    fn propagate_block_light_decrease(
        &self,
        level: &Arc<Level>,
        pos: &BlockPos,
        removed_light_level: u8,
    ) {
        // 检查该位置当前的光照等级实际是多少
        let current_level = self.get_block_light_level(level, pos).unwrap_or(0);

        // 仅当此位置尚未被重置为 0 时才传播减少
        // 这防止被有意设为 0 的位置传播光照
        if current_level == 0 && removed_light_level > 0 {
            // 此位置已被调暗，因此我们将黑暗传播给邻居
            for dir in BlockDirection::all() {
                let neighbor_pos = pos.offset(dir.to_offset());

                if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                    continue;
                }

                if let Some(neighbor_light) = self.get_block_light_level(level, &neighbor_pos) {
                    if neighbor_light == 0 {
                        continue; // 若已为 0 则跳过
                    }

                    let neighbor_state = level.get_block_state(&neighbor_pos).to_state();
                    let opacity = neighbor_state.opacity.max(1);

                    let expected_from_removed_source = removed_light_level.saturating_sub(opacity);

                    if neighbor_light <= expected_from_removed_source {
                        let neighbor_luminance = neighbor_state.luminance;

                        if neighbor_luminance == 0 {
                            // 无自发光，将其完全变暗并继续传播
                            self.set_block_light_level(level, &neighbor_pos, 0).ok();
                            self.queue_block_light_decrease(neighbor_pos, neighbor_light);
                        } else {
                            // 具有自发光：将其设为自身亮度并从此处重新传播
                            self.set_block_light_level(level, &neighbor_pos, neighbor_luminance)
                                .ok();
                            self.queue_block_light_increase(neighbor_pos, neighbor_luminance);
                        }
                    } else {
                        // 这个邻居从其他光源获得了更亮的光照，从它重新传播
                        self.queue_block_light_increase(neighbor_pos, neighbor_light);
                    }
                }
            }
        }
    }

    pub fn check_block_light_updates(&self, level: &Arc<Level>, pos: BlockPos) {
        match level.lighting_config {
            LightingEngineConfig::Full => {
                self.set_block_light_level(level, &pos, 15).ok();
                return;
            }
            LightingEngineConfig::Dark => {
                self.set_block_light_level(level, &pos, 0).ok();
                return;
            }
            LightingEngineConfig::Default => {}
        }

        let current_light = self.get_block_light_level(level, &pos).unwrap_or(0);
        let block_state = level.get_block_state(&pos).to_state();
        let expected_light = block_state.luminance;

        // 处理光照降低（移除光源或放置不透明方块）
        if expected_light < current_light {
            // 立即设为期望值，然后将递减操作排队以使邻居变暗
            self.set_block_light_level(level, &pos, expected_light).ok();
            self.queue_block_light_decrease(pos, current_light);
        } else if expected_light > current_light {
            // 处理光照增加（放置光源）
            self.set_block_light_level(level, &pos, expected_light).ok();
            self.queue_block_light_increase(pos, expected_light);
        }

        // 只有在未触发减少时才检查邻居
        // 衰减传播会重新校验相邻方块
        if expected_light >= current_light {
            self.check_neighbors_light_updates(level, pos, expected_light);
        }
    }

    pub fn check_neighbors_light_updates(
        &self,
        level: &Arc<Level>,
        pos: BlockPos,
        current_light: u8,
    ) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }
            if let Some(neighbor_light) = self.get_block_light_level(level, &neighbor_pos)
                && neighbor_light > current_light + 1
            {
                self.queue_block_light_increase(neighbor_pos, neighbor_light);
            }
        }
    }

    pub fn perform_sky_light_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;
        loop {
            let decrease_updates = self.perform_sky_light_decrease_updates(level);
            let increase_updates = self.perform_sky_light_increase_updates(level);

            updates += decrease_updates + increase_updates;

            if decrease_updates == 0 && increase_updates == 0 {
                break;
            }
        }
        updates
    }

    fn perform_sky_light_decrease_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;
        while let Some((pos, expected_light)) = self.sky_decrease.pop() {
            self.propagate_sky_light_decrease(level, &pos, expected_light);
            updates += 1;
        }
        updates
    }

    fn perform_sky_light_increase_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;
        while let Some((pos, expected_light)) = self.sky_increase.pop() {
            self.propagate_sky_light_increase(level, &pos, expected_light);
            updates += 1;
        }
        updates
    }

    fn propagate_sky_light_increase(&self, level: &Arc<Level>, pos: &BlockPos, light_level: u8) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }

            // 永不传播到未加载的区块。对未加载
            // 区块会被静默丢弃，因此“比邻居更亮”检查
            // 下方条件将永远为真并不断重新入队相同的
            // 位置，使这个循环在两者交界处无限空转
            // 已加载与已卸载的区块。
            let (neighbor_chunk, _) = neighbor_pos.chunk_and_chunk_relative_position();
            if !level.is_chunk_loaded(&neighbor_chunk) {
                continue;
            }

            let neighbor_light = self.get_sky_light_level(level, &neighbor_pos);
            let neighbor_state = level.get_block_state(&neighbor_pos).to_state();
            let opacity = if neighbor_state.can_occlude() {
                neighbor_state.opacity.max(1)
            } else {
                neighbor_state.opacity
            };

            // 计算邻居的新光照等级
            let new_light = if light_level == 15
                && dir == BlockDirection::Down
                && opacity == 0
                && !neighbor_state.can_occlude()
            {
                // 特殊情况：15 级天空光照可透过透明方块以 15 级向下传播
                15
            } else {
                // 正常传播：先按距离减 1，再按不透明度削减
                light_level.saturating_sub(1).saturating_sub(opacity)
            };

            // 仅当新光比当前光更亮时才传播
            if new_light > neighbor_light {
                self.set_sky_light_level(level, &neighbor_pos, new_light)
                    .ok();

                if new_light > 0 {
                    self.queue_sky_light_increase(neighbor_pos, new_light);
                }
            }
        }
    }

    fn propagate_sky_light_decrease(&self, level: &Arc<Level>, pos: &BlockPos, removed_light: u8) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }

            // 见 `propagate_sky_light_increase`：跳过未加载的区块，以便天空
            // 光照更新绝不会在已加载/未加载区块的边界处空转。
            let (neighbor_chunk, _) = neighbor_pos.chunk_and_chunk_relative_position();
            if !level.is_chunk_loaded(&neighbor_chunk) {
                continue;
            }

            let neighbor_light = self.get_sky_light_level(level, &neighbor_pos);
            if neighbor_light == 0 {
                continue; // 已经是暗的
            }

            let neighbor_state = level.get_block_state(&neighbor_pos).to_state();
            let opacity = if neighbor_state.can_occlude() {
                neighbor_state.opacity.max(1)
            } else {
                neighbor_state.opacity
            };

            // 计算本会给予该邻居的数量
            let expected = if removed_light == 15
                && dir == BlockDirection::Down
                && opacity == 0
                && !neighbor_state.can_occlude()
            {
                15
            } else {
                removed_light.saturating_sub(1).saturating_sub(opacity)
            };

            if neighbor_light == expected || neighbor_light < removed_light {
                // 这个邻居的光是由我们提供的，将其调暗
                self.set_sky_light_level(level, &neighbor_pos, 0).ok();
                self.queue_sky_light_decrease(neighbor_pos, neighbor_light);
            } else if neighbor_light > removed_light {
                // 邻居有来自其他光源的更亮光
                // 从它重新传播，以填补我们留下的空隙
                self.queue_sky_light_increase(neighbor_pos, neighbor_light);
            }
        }
    }

    pub fn check_sky_light_updates(&self, level: &Arc<Level>, pos: BlockPos) {
        match level.lighting_config {
            LightingEngineConfig::Full => {
                self.set_sky_light_level(level, &pos, 15).ok();
                return;
            }
            LightingEngineConfig::Dark => {
                self.set_sky_light_level(level, &pos, 0).ok();
                return;
            }
            LightingEngineConfig::Default => {}
        }

        let current_light = self.get_sky_light_level(level, &pos);
        let block_state = level.get_block_state(&pos).to_state();
        let opacity = if block_state.can_occlude() {
            block_state.opacity.max(1)
        } else {
            block_state.opacity
        };

        // 计算预期的天空光照
        let expected_light = if opacity == 15 || block_state.is_solid_render() {
            // 完全不透明的方块 = 无光
            0
        } else {
            // 检查上方是否露天
            let has_sky = self.has_open_sky_above(level, &pos);

            if has_sky {
                // 直射阳光，按不透明度衰减
                15u8.saturating_sub(opacity)
            } else {
                // 无直接天空光照，检查邻居获取最佳光照
                let mut best_light = 0;

                for dir in BlockDirection::all() {
                    let neighbor_pos = pos.offset(dir.to_offset());

                    if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                        continue;
                    }

                    let neighbor_light = self.get_sky_light_level(level, &neighbor_pos);
                    // 计算来自该邻居的潜在光照
                    let potential = if neighbor_light == 15
                        && dir == BlockDirection::Up
                        && opacity == 0
                        && !block_state.can_occlude()
                    {
                        // 来自上方的 15 级天空光照可透过不遮挡的透明方块，保持 15 级
                        15
                    } else {
                        // 正常腐烂
                        neighbor_light.saturating_sub(1)
                    };

                    best_light = best_light.max(potential);
                }

                // 对最佳入射光应用不透明度
                best_light.saturating_sub(opacity)
            }
        };

        // 如有需要则更新
        if expected_light < current_light {
            // 光照减弱
            self.set_sky_light_level(level, &pos, expected_light).ok();
            self.queue_sky_light_decrease(pos, current_light);
        } else if expected_light > current_light {
            // 光照增强
            self.set_sky_light_level(level, &pos, expected_light).ok();
            self.queue_sky_light_increase(pos, expected_light);
        }

        // 若光照增强或保持不变则通知邻居
        if expected_light >= current_light {
            self.check_neighbors_sky_light_updates(pos, expected_light);
        }
    }

    pub fn check_neighbors_sky_light_updates(&self, pos: BlockPos, current_light: u8) {
        // 更新位置时，向邻居传播
        if current_light > 0 {
            self.queue_sky_light_increase(pos, current_light);
        }
    }

    pub fn get_block_light_level_sync(&self, level: &Level, position: &BlockPos) -> Option<u8> {
        let (chunk_pos, relative) = position.chunk_and_chunk_relative_position();

        level.read_chunk_sync(&chunk_pos, |chunk| {
            let section_idx = (relative.y - chunk.section.min_y) as usize / 16;
            let light_engine = chunk.light_engine.lock().ok()?;

            light_engine
                .block_light
                .get(section_idx)?
                .get(
                    relative.x as usize,
                    (relative.y - chunk.section.min_y) as usize % 16,
                    relative.z as usize,
                )
                .into()
        })?
    }

    pub fn get_sky_light_level_sync(&self, level: &Level, position: &BlockPos) -> u8 {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        level
            .read_chunk_sync(&chunk_coordinate, |chunk| {
                let section_index =
                    (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;

                let light_engine = chunk
                    .light_engine
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                // 区块段索引的边界检查（锁定光照引擎）
                if section_index >= light_engine.sky_light.len() {
                    return 15;
                }

                light_engine.sky_light[section_index].get(
                    relative.x as usize,
                    (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE,
                    relative.z as usize,
                )
            })
            .unwrap_or(0)
    }

    pub fn get_block_light_level(&self, level: &Arc<Level>, position: &BlockPos) -> Option<u8> {
        let (chunk_pos, relative) = position.chunk_and_chunk_relative_position();

        level
            .read_chunk_sync(&chunk_pos, |chunk| {
                let section_idx = (relative.y - chunk.section.min_y) as usize / 16;
                chunk
                    .light_engine
                    .lock()
                    .ok()?
                    .block_light
                    .get(section_idx)
                    .map(|section| {
                        section.get(
                            relative.x as usize,
                            (relative.y - chunk.section.min_y) as usize % 16,
                            relative.z as usize,
                        )
                    })
            })
            .flatten()
    }

    pub fn set_block_light_level(
        &self,
        level: &Arc<Level>,
        position: &BlockPos,
        light_level: u8,
    ) -> Result<(), String> {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        level.read_chunk_sync(&chunk_coordinate, |chunk| {
            let section_index = (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;
            {
                let mut light_engine = chunk
                    .light_engine
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if section_index >= light_engine.block_light.len() {
                    return Err("Invalid section index".to_string());
                }
                let relative_y = (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE;
                light_engine.block_light[section_index].set(
                    relative.x as usize,
                    relative_y,
                    relative.z as usize,
                    light_level,
                );
            };
            // 将区块标记为脏，以便光照变化保存到磁盘
            if !chunk.is_dirty() {
                chunk.mark_dirty(true);
            }
            Ok(())
        });
        Ok(())
    }

    pub fn get_sky_light_level(&self, level: &Arc<Level>, position: &BlockPos) -> u8 {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        level
            .read_chunk_sync(&chunk_coordinate, |chunk| {
                let section_index =
                    (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;

                let light_engine = chunk
                    .light_engine
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                // 区块段索引的边界检查（锁定光照引擎）
                if section_index >= light_engine.sky_light.len() {
                    return 15;
                }

                light_engine.sky_light[section_index].get(
                    relative.x as usize,
                    (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE,
                    relative.z as usize,
                )
            })
            .unwrap_or(0)
    }

    pub fn set_sky_light_level(
        &self,
        level: &Arc<Level>,
        position: &BlockPos,
        light_level: u8,
    ) -> Result<(), String> {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        level.read_chunk_sync(&chunk_coordinate, |chunk| {
            let section_index = (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;
            {
                let mut light_engine = chunk
                    .light_engine
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if section_index >= light_engine.sky_light.len() {
                    return Err("Invalid section index".to_string());
                }
                let relative_y = (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE;
                light_engine.sky_light[section_index].set(
                    relative.x as usize,
                    relative_y,
                    relative.z as usize,
                    light_level,
                );
            };
            // 将区块标记为脏，以便光照变化保存到磁盘
            if !chunk.is_dirty() {
                chunk.mark_dirty(true);
            }
            Ok(())
        });
        Ok(())
    }
}
