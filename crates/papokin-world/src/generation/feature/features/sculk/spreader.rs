//! 幽匿蔓延：电荷光标转化方块并放置生长物。

use papokin_data::BlockDirection;
use papokin_data::BlockId;
use papokin_data::BlockStateId;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_util::random::RandomGenerator;
use papokin_util::random::RandomImpl;
use std::collections::HashMap;

use super::NON_CORNER_NEIGHBOURS;
use super::SculkLevel;
use super::growth::GrowthRules;
use super::is_sculk_behaviour;
use super::is_sculk_replaceable;
use super::is_sculk_replaceable_world_gen;
use super::vein::VeinRules;

/// 同时存在的光标最大数量。
pub const MAX_CURSORS: usize = 32;
/// 单个光标可携带的最大充能值。
pub const MAX_CHARGE: u16 = 1000;
/// 光标被丢弃前与原点的最大棋盘距离。
const MAX_CURSOR_DISTANCE: i32 = 1024;
/// 世界生成期间的 XZ 半径平方限制：超出后游标会被丢弃
/// 一旦 `dx² + dz² >= 225`（距原点 15 格）。
const WORLD_GEN_RADIUS_SQ: i64 = 225;

/// 从 [`SculkSpreader`] 提取的配置值，使光标
/// 更新不需要在迭代时借用传播器。
#[derive(Clone, Copy)]
pub struct SpreaderConfig {
    pub is_world_generation: bool,
    pub growth_spawn_cost: i32,
    pub no_growth_radius: i32,
    pub charge_decay_rate: i32,
    pub additional_decay_rate: i32,
    /// 可替换方块是否通过世界生成标签解析
    /// (`minecraft:sculk_replaceable_world_gen`) 而非标签
    /// (`minecraft:sculk_replaceable`)。
    pub replaceable_world_gen: bool,
}

impl SculkSpreader {
    ///返回用于光标更新的配置快照。
    #[must_use]
    pub const fn config(&self) -> SpreaderConfig {
        SpreaderConfig {
            is_world_generation: self.is_world_generation,
            growth_spawn_cost: self.growth_spawn_cost,
            no_growth_radius: self.no_growth_radius,
            charge_decay_rate: self.charge_decay_rate,
            additional_decay_rate: self.additional_decay_rate,
            replaceable_world_gen: self.replaceable_tag_world_gen,
        }
    }
}

/// 驱动单个幽匿斑块的蔓延。
pub struct SculkSpreader {
    /// 此蔓延器是否在世界生成期间运行（而非由催化剂驱动）。
    is_world_generation: bool,
    /// 用于确定哪些方块可被替换的标签。
    replaceable_tag_world_gen: bool,
    /// 生成一个生长体（感测体/尖啸体）所需的电荷成本。
    growth_spawn_cost: i32,
    /// 原点周围不会生成生长物的半径。
    no_growth_radius: i32,
    /// 充能衰减率——`random.nextInt(rate) == 0` 时触发衰减。
    charge_decay_rate: i32,
    /// 没有生长生成时的额外衰减率。
    additional_decay_rate: i32,
    /// 活动光标。数量受限于 [`MAX_CURSORS`]。
    cursors: Vec<ChargeCursor>,
}

impl SculkSpreader {
    /// 创建用于催化剂驱动（在世界内）扩散的扩散器。
    #[must_use]
    pub const fn new_level_spreader() -> Self {
        Self::new(false, false, 10, 4, 10, 5)
    }

    /// 创建用于世界生成扩散的扩散器。
    #[must_use]
    pub const fn new_world_gen_spreader() -> Self {
        Self::new(true, true, 50, 1, 5, 10)
    }

    #[must_use]
    const fn new(
        is_world_generation: bool,
        replaceable_tag_world_gen: bool,
        growth_spawn_cost: i32,
        no_growth_radius: i32,
        charge_decay_rate: i32,
        additional_decay_rate: i32,
    ) -> Self {
        Self {
            is_world_generation,
            replaceable_tag_world_gen,
            growth_spawn_cost,
            no_growth_radius,
            charge_decay_rate,
            additional_decay_rate,
            cursors: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_world_generation(&self) -> bool {
        self.is_world_generation
    }

    #[inline]
    #[must_use]
    pub const fn growth_spawn_cost(&self) -> i32 {
        self.growth_spawn_cost
    }

    #[inline]
    #[must_use]
    pub const fn no_growth_radius(&self) -> i32 {
        self.no_growth_radius
    }

    #[inline]
    #[must_use]
    pub const fn charge_decay_rate(&self) -> i32 {
        self.charge_decay_rate
    }

    #[inline]
    #[must_use]
    pub const fn additional_decay_rate(&self) -> i32 {
        self.additional_decay_rate
    }

    ///若给定的方块 ID 对该蔓延器而言是可替换的，则返回 `true`。
    #[inline]
    #[must_use]
    pub fn is_replaceable(&self, id: BlockId) -> bool {
        if self.replaceable_tag_world_gen {
            super::is_sculk_replaceable_world_gen(id)
        } else {
            super::is_sculk_replaceable(id)
        }
    }

    /// 在某个位置添加电荷，若
    /// 蓄力值超过 [`MAX_CHARGE`]。
    pub fn add_cursors(&mut self, start_pos: BlockPos, mut charge: i32) {
        while charge > 0 {
            let current = std::cmp::min(charge, MAX_CHARGE as i32);
            self.add_cursor(ChargeCursor::new(start_pos, current as u16));
            charge -= current;
        }
    }

    /// 添加单个光标，遵守 [`MAX_CURSORS`] 限制。
    fn add_cursor(&mut self, cursor: ChargeCursor) {
        if self.cursors.len() < MAX_CURSORS {
            self.cursors.push(cursor);
        }
    }

    /// 清除所有光标（在各轮之间调用）。
    pub fn clear(&mut self) {
        self.cursors.clear();
    }

    ///返回对当前活动光标的引用（用于测试）。
    #[cfg(test)]
    #[must_use]
    pub fn cursors(&self) -> &[ChargeCursor] {
        &self.cursors
    }

    /// 遍历所有活跃光标的主更新循环。
    pub fn update_cursors(
        &mut self,
        level: &mut dyn SculkLevel,
        origin_pos: BlockPos,
        random: &mut RandomGenerator,
        spread_veins: bool,
    ) {
        if self.cursors.is_empty() {
            return;
        }

        let mut processed: Vec<ChargeCursor> = Vec::with_capacity(self.cursors.len());
        // 合并映射：位置 -> `processed` 中的索引。
        let mut merge_index: HashMap<BlockPos, usize> = HashMap::with_capacity(self.cursors.len());

        let config = self.config();
        for cursor in self.cursors.drain(..) {
            if cursor.pos.0.x.abs_diff(origin_pos.0.x) > MAX_CURSOR_DISTANCE as u32
                || cursor.pos.0.y.abs_diff(origin_pos.0.y) > MAX_CURSOR_DISTANCE as u32
                || cursor.pos.0.z.abs_diff(origin_pos.0.z) > MAX_CURSOR_DISTANCE as u32
            {
                continue; // 无法到达的位置，丢弃
            }

            let mut cursor = cursor;
            cursor.update(level, origin_pos, random, config, spread_veins);

            if cursor.charge == 0 {
                continue;
            }

            let pos = cursor.pos;
            // 尝试与同一位置上已有的光标合并。
            if let Some(existing_idx) = merge_index.get(&pos).copied() {
                let existing_charge = processed[existing_idx].charge;
                // 非世界生成且合并后的电荷可容纳时进行合并
                // 在 MAX_CHARGE 范围内。
                if !config.is_world_generation {
                    let combined = existing_charge as u32 + cursor.charge as u32;
                    if combined <= MAX_CHARGE as u32 {
                        processed[existing_idx].charge = combined as u16;
                        processed[existing_idx].update_delay = processed[existing_idx]
                            .update_delay
                            .min(cursor.update_delay);
                        continue;
                    }
                }
                // 无法合并：将新游标保留为独立条目。它
                // 特意不注册进 merge_index——
                // 已有条目必须仍是后续合并的目标
                // 查找——除非它携带的电荷少于
                // 已存在的游标，此时合并目标为
                // 被新游标取代。
                processed.push(cursor);
                let new_idx = processed.len() - 1;
                if processed[new_idx].charge < existing_charge {
                    merge_index.insert(pos, new_idx);
                }
            } else {
                merge_index.insert(pos, processed.len());
                processed.push(cursor);
            }
        }

        self.cursors = processed;
    }
}

/// 单个“光标”——幽匿充能的一个移动点。
pub struct ChargeCursor {
    pub pos: BlockPos,
    pub charge: u16,
    pub update_delay: u8,
    pub decay_delay: u8,
    /// 此光标各面的可选位集。`None` 表示同一空间
    /// 传播专用，`Some(0)` 表示空集，`Some(bits)` 保存
    /// 朝向位（位 0 = Down … 位 5 = East）。
    pub faces: Option<u8>,
}

impl ChargeCursor {
    #[must_use]
    pub const fn new(pos: BlockPos, charge: u16) -> Self {
        Self {
            pos,
            charge,
            update_delay: 0,
            decay_delay: 1,
            faces: None,
        }
    }

    ///返回面位集中已设置的 `BlockDirection` 位。
    pub fn facing_directions(&self) -> impl Iterator<Item = BlockDirection> + '_ {
        let bits = self.faces.unwrap_or(0);
        BlockDirection::all()
            .into_iter()
            .filter(move |dir| bits & (1 << dir.to_index()) != 0)
    }

    /// 单个光标的每刻核心更新逻辑。
    pub fn update(
        &mut self,
        level: &mut dyn SculkLevel,
        origin_pos: BlockPos,
        random: &mut RandomGenerator,
        config: SpreaderConfig,
        spread_veins: bool,
    ) {
        if self.charge == 0 {
            return;
        }
        if self.update_delay > 0 {
            self.update_delay -= 1;
            return;
        }

        let mut current_state = level.sculk_get(self.pos);
        let mut current_id = current_state.map_or(BlockId::AIR, BlockStateId::to_block_id);

        // 先尝试脉络蔓延。Sculk 行为方块（sculk /
        // 幽匿脉）使用多面 `spread_all`，而所有其他
        // 方块根据光标的朝向（`None` → 同一空间，
        // 为空 → `spread_all`，非空 → 再生）。
        if spread_veins {
            let spread = if is_sculk_behaviour(current_id) {
                VeinRules::spread_all(level, self.pos)
            } else {
                VeinRules::attempt_spread_vein(level, self.pos, current_state, self.faces)
            };
            // 在成功扩散后重新读取状态，除非
            // 幽匿：只有幽匿方块在被蔓延覆盖时不会改变状态。
            if spread && current_id != BlockId::SCULK {
                current_state = level.sculk_get(self.pos);
                current_id = current_state.map_or(BlockId::AIR, BlockStateId::to_block_id);
            }
        }

        // 应用充能衰减/生长逻辑。
        self.charge = Self::attempt_use_charge(
            self,
            current_id,
            level,
            origin_pos,
            random,
            &config,
            spread_veins,
        );

        if self.charge == 0 {
            // 放电时只有幽匿脉络会移除面；其他所有
            // 方块类型为空操作（no-op）。
            if current_id == BlockId::SCULK_VEIN {
                VeinRules::on_discharged(level, self.pos);
            }
            return;
        }

        // 尝试移动。
        if let Some(new_pos) = Self::valid_movement_position(level, self.pos, random) {
            if current_id == BlockId::SCULK_VEIN {
                VeinRules::on_discharged(level, self.pos);
            }
            self.pos = new_pos;

            // 世界生成半径限制（仅 XZ）：`dx² + dz² >= 225`。
            if config.is_world_generation {
                let dx = (self.pos.0.x - origin_pos.0.x) as i64;
                let dz = (self.pos.0.z - origin_pos.0.z) as i64;
                if dx * dx + dz * dz >= WORLD_GEN_RADIUS_SQ {
                    self.charge = 0;
                    return;
                }
            }

            // 根据新位置处的方块更新各面。
            if let Some(state) = level.sculk_get(self.pos) {
                let id = state.to_block_id();
                if is_sculk_behaviour(id) {
                    self.faces = Some(Self::available_faces(state, id));
                }
            }
        }

        // 更新延迟。
        // 衰减延迟使用移动前的方块行为：
        // 幽匿行为会将其重置为 1，其他行为则将其递减。
        self.decay_delay =
            Self::update_decay_delay(self.decay_delay, is_sculk_behaviour(current_id));
        self.update_delay = 1; // 传播延迟始终为单个刻
    }

    /// 决定本刻消耗多少充能，并按具体类型分派
    /// 光标位置处的方块：
    /// - 幽匿脉络 → 按下方的转化或减半规则处理
    /// - 幽匿块 → 进行生长放置，并按距离衰减
    /// - 其余所有内容（包括幽匿感测体、幽匿尖啸体和幽匿催发体）→
    ///   在消退延迟期间保持电荷，否则放电
    fn attempt_use_charge(
        &self,
        current_id: BlockId,
        level: &mut dyn SculkLevel,
        origin_pos: BlockPos,
        random: &mut RandomGenerator,
        config: &SpreaderConfig,
        spread_veins: bool,
    ) -> u16 {
        let charge = self.charge;
        if charge == 0 {
            return 0;
        }
        match current_id {
            BlockId::SCULK_VEIN => {
                let replaceable = |id: BlockId| {
                    if config.replaceable_world_gen {
                        is_sculk_replaceable_world_gen(id)
                    } else {
                        is_sculk_replaceable(id)
                    }
                };
                if spread_veins
                    && VeinRules::attempt_place_sculk(level, self.pos, random, replaceable)
                {
                    return charge.saturating_sub(1);
                }
                if random.next_bounded_i32(config.charge_decay_rate) == 0 {
                    // `Mth.floor(charge * 0.5F)` —— 把蓄力值减半。
                    charge / 2
                } else {
                    charge
                }
            }
            BlockId::SCULK => {
                Self::sculk_block_use_charge(self.pos, level, origin_pos, random, config, charge)
            }
            // 默认规则：`decay_delay > 0 ? charge : 0`。
            _ => {
                if self.decay_delay > 0 {
                    charge
                } else {
                    0
                }
            }
        }
    }

    /// 幽匿块的生长放置与基于距离的电荷衰减。
    fn sculk_block_use_charge(
        pos: BlockPos,
        level: &mut dyn SculkLevel,
        origin_pos: BlockPos,
        random: &mut RandomGenerator,
        config: &SpreaderConfig,
        charge: u16,
    ) -> u16 {
        if charge == 0 {
            return 0;
        }
        // 掷骰决定腐化。
        if random.next_bounded_i32(config.charge_decay_rate) != 0 {
            return charge;
        }

        let is_close_to_catalyst =
            Self::is_close_to_catalyst(pos, origin_pos, config.no_growth_radius);

        if !is_close_to_catalyst && GrowthRules::can_place_growth(level, pos) {
            let xp_per_growth = config.growth_spawn_cost;
            if random.next_bounded_i32(xp_per_growth) < charge as i32 {
                let growth_pos = pos.up();
                let growth_state = GrowthRules::random_growth_state(
                    level,
                    growth_pos,
                    random,
                    config.is_world_generation,
                );
                level.sculk_set(growth_pos, growth_state);
            }
            // 消耗充能（`Math.max(0, charge - xpPerGrowthSpawn)`）。
            return charge.saturating_sub(xp_per_growth as u16);
        }

        // 无生长 — 施加额外腐烂或小幅递减。
        if random.next_bounded_i32(config.additional_decay_rate) != 0 {
            return charge;
        }

        if is_close_to_catalyst {
            charge.saturating_sub(1)
        } else {
            let penalty = Self::decay_penalty(config, pos, origin_pos, charge as i32);
            charge.saturating_sub(penalty as u16)
        }
    }

    /// 严格的平方欧氏距离贴近度比较。
    const fn is_close_to_catalyst(
        pos: BlockPos,
        origin_pos: BlockPos,
        no_growth_radius: i32,
    ) -> bool {
        let dx = (pos.0.x - origin_pos.0.x) as i64;
        let dy = (pos.0.y - origin_pos.0.y) as i64;
        let dz = (pos.0.z - origin_pos.0.z) as i64;
        let radius = no_growth_radius as i64;
        dx * dx + dy * dy + dz * dz < radius * radius
    }

    /// 基于距离的充能衰减惩罚。
    fn decay_penalty(
        config: &SpreaderConfig,
        pos: BlockPos,
        origin_pos: BlockPos,
        charge: i32,
    ) -> i32 {
        let no_growth_radius = config.no_growth_radius;
        let dx = (pos.0.x - origin_pos.0.x) as f64;
        let dy = (pos.0.y - origin_pos.0.y) as f64;
        let dz = (pos.0.z - origin_pos.0.z) as f64;
        // 距离先以双精度计算，然后再转换为
        // 转为浮点数后再平方（与参考实现的行为一致）。
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        let outer_distance_sq = (distance as f32 - no_growth_radius as f32).powi(2);
        // `Mth.square(24 - noGrowthRadius)` 是一个 int。
        let max_reach_sq = (24 - no_growth_radius).pow(2);
        let factor = (outer_distance_sq / max_reach_sq as f32).min(1.0);
        // `(int)(charge * distanceFactor * 0.5F)` —— 向零截断。
        let penalty = (charge as f32 * factor * 0.5) as i32;
        penalty.max(1)
    }

    /// 扫描非角落的邻近方块以查找具有幽匿行为的方块，
    /// 光标可移动到的位置。
    fn valid_movement_position(
        level: &dyn SculkLevel,
        pos: BlockPos,
        random: &mut RandomGenerator,
    ) -> Option<BlockPos> {
        let mut result = None;

        // 打乱 18 个非角落的邻居（Fisher-Yates）。
        let mut order: [u8; 18] = core::array::from_fn(|i| i as u8);
        for i in (1..18).rev() {
            let j = random.next_bounded_i32((i + 1) as i32) as usize;
            order.swap(i, j);
        }

        for &idx in &order {
            let offset = NON_CORNER_NEIGHBOURS[idx as usize];
            let neighbour = pos.offset(offset);

            let Some(state) = level.sculk_get(neighbour) else {
                continue;
            };
            let id = state.to_block_id();
            if !is_sculk_behaviour(id) {
                continue;
            }
            if !Self::is_movement_unobstructed(level, pos, neighbour) {
                continue;
            }
            // 在每个有效邻居上覆盖候选值，并且只
            // 在访问基底时提前停止，因此没有基底时
            // 以最后一个有效邻接为准。
            result = Some(neighbour);
            if VeinRules::has_substrate_access(level, state, neighbour) {
                // 找到了可接触基底的目标——立即采用。
                break;
            }
        }

        // 未找到有效目标时返回 `None`。
        result
    }

    /// 检查两个相邻偏移之间的移动是否被阻挡。
    fn is_movement_unobstructed(level: &dyn SculkLevel, from: BlockPos, to: BlockPos) -> bool {
        let delta = Vector3::new(to.0.x - from.0.x, to.0.y - from.0.y, to.0.z - from.0.z);
        // 曼哈顿距离 == 1 → 永远无遮挡。
        if delta.x.abs() + delta.y.abs() + delta.z.abs() == 1 {
            return true;
        }
        let dir_x = if delta.x < 0 {
            BlockDirection::West
        } else {
            BlockDirection::East
        };
        let dir_y = if delta.y < 0 {
            BlockDirection::Down
        } else {
            BlockDirection::Up
        };
        let dir_z = if delta.z < 0 {
            BlockDirection::North
        } else {
            BlockDirection::South
        };
        if delta.x == 0 {
            Self::is_unobstructed(level, from, dir_y) || Self::is_unobstructed(level, from, dir_z)
        } else if delta.y == 0 {
            Self::is_unobstructed(level, from, dir_x) || Self::is_unobstructed(level, from, dir_z)
        } else {
            Self::is_unobstructed(level, from, dir_x) || Self::is_unobstructed(level, from, dir_y)
        }
    }

    fn is_unobstructed(level: &dyn SculkLevel, from: BlockPos, direction: BlockDirection) -> bool {
        let test_pos = from.offset(direction.to_offset());
        !level.sculk_is_face_sturdy(test_pos, direction.opposite())
    }

    /// 从方块状态自身的面派生出面位集
    /// 属性。只有多面方块（幽匿脉络）才会暴露面；
    /// 其余所有方块都会得到空位集。
    fn available_faces(state: BlockStateId, id: BlockId) -> u8 {
        if id != BlockId::SCULK_VEIN {
            return 0;
        }
        let mut faces: u8 = 0;
        for dir in BlockDirection::all() {
            if VeinRules::has_face(state, dir) {
                faces |= 1 << dir.to_index();
            }
        }
        faces
    }

    /// 衰减延迟更新：幽匿行为会将延迟重置为 1，
    /// 其他事件则将其递减（下限饱和到 0）。
    const fn update_decay_delay(current: u8, is_sculk: bool) -> u8 {
        if is_sculk {
            1
        } else {
            current.saturating_sub(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::Block;
    use papokin_util::random::legacy_rand::LegacyRand;

    use crate::generation::feature::features::sculk::test_utils::MockSculkLevel;

    #[test]
    fn non_corner_neighbours_count() {
        assert_eq!(NON_CORNER_NEIGHBOURS.len(), 18);
    }

    #[test]
    fn non_corner_neighbours_valid() {
        for off in NON_CORNER_NEIGHBOURS {
            // 至少一个轴必须为零（非角点）。
            let zeros = (off.x == 0) as u8 + (off.y == 0) as u8 + (off.z == 0) as u8;
            assert!(zeros >= 1, "corner offset found: {off:?}");
            // 不是中心。
            assert!(
                !(off.x == 0 && off.y == 0 && off.z == 0),
                "centre offset found"
            );
            // 每个轴在 -1..=1 范围内
            assert!(off.x >= -1 && off.x <= 1);
            assert!(off.y >= -1 && off.y <= 1);
            assert!(off.z >= -1 && off.z <= 1);
        }
    }

    #[test]
    fn charge_saturation() {
        let mut s = SculkSpreader::new_world_gen_spreader();
        let origin = BlockPos::new(0, 60, 0);
        s.add_cursors(origin, 2500);
        // 2500 = 1000 + 1000 + 500 → 3 个游标。
        assert_eq!(s.cursors().len(), 3);
        assert_eq!(s.cursors()[0].charge, 1000);
        assert_eq!(s.cursors()[1].charge, 1000);
        assert_eq!(s.cursors()[2].charge, 500);
    }

    #[test]
    fn max_cursors_limit() {
        let mut s = SculkSpreader::new_world_gen_spreader();
        let origin = BlockPos::new(0, 60, 0);
        for _ in 0..50 {
            s.add_cursors(origin, 1000);
        }
        assert!(s.cursors().len() <= MAX_CURSORS);
    }

    #[test]
    fn no_underflow() {
        let mut c = ChargeCursor::new(BlockPos::new(0, 60, 0), 0);
        assert_eq!(c.charge, 0);
        // DEFAULT 行为使用 decay_delay saturating_sub。
        c.decay_delay = 0;
        assert_eq!(ChargeCursor::update_decay_delay(c.decay_delay, false), 0);
        // 幽匿行为会将延迟重置为 1。
        assert_eq!(ChargeCursor::update_decay_delay(0, true), 1);
    }

    #[test]
    fn cursor_faces_default_to_none() {
        let c = ChargeCursor::new(BlockPos::new(0, 60, 0), 100);
        assert_eq!(c.faces, None);
        assert_eq!(c.facing_directions().count(), 0);
    }

    #[test]
    fn close_to_catalyst_is_strict() {
        let origin = BlockPos::new(0, 60, 0);
        // `closerThan` 使用 `< radius²`：恰好在边界上并不算接近。
        assert!(!ChargeCursor::is_close_to_catalyst(
            BlockPos::new(4, 60, 0),
            origin,
            4,
        ));
        assert!(ChargeCursor::is_close_to_catalyst(
            BlockPos::new(3, 60, 0),
            origin,
            4,
        ));
        // 3D 距离：(2, 2, 2) → √12 < 4。
        assert!(ChargeCursor::is_close_to_catalyst(
            BlockPos::new(2, 62, 2),
            origin,
            4,
        ));
    }

    #[test]
    fn available_faces_reads_block_state() {
        // 只有多面（幽匿脉络）状态才会暴露面。
        let base = Block::SCULK_VEIN.default_state.id;
        assert_eq!(ChargeCursor::available_faces(base, BlockId::SCULK_VEIN), 0,);
        let with_up = VeinRules::with_face(base, BlockDirection::Up, true);
        assert_eq!(
            ChargeCursor::available_faces(with_up, BlockId::SCULK_VEIN),
            1 << BlockDirection::Up.to_index(),
        );
        // 非 multiface 状态总是产生空集合。
        assert_eq!(
            ChargeCursor::available_faces(Block::SCULK.default_state.id, BlockId::SCULK,),
            0,
        );
    }

    #[test]
    fn cursor_does_not_move_onto_sensor() {
        // 只有 `SculkBehaviour` 方块才是有效的移动目标；
        // 而单个传感器邻居则不会。
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos, Block::SCULK.default_state.id);
        level.set_id(pos.up(), Block::SCULK_SENSOR.default_state.id);
        let mut random = RandomGenerator::Legacy(LegacyRand::from_seed(1));
        assert_eq!(
            ChargeCursor::valid_movement_position(&level, pos, &mut random),
            None,
        );
    }

    #[test]
    fn cursor_moves_to_last_valid_neighbour_without_substrate() {
        // 候选值会在每个有效邻居处被覆盖，而且
        // 扫描仅在触及基底时提前停止，因此没有基底时
        // 乱序后最后一个合法邻居获胜
        // （种子 1 会在 `east` 之后打乱 `up`）。
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos, Block::SCULK.default_state.id);
        level.set_id(pos.up(), Block::SCULK.default_state.id);
        level.set_id(pos.east(), Block::SCULK.default_state.id);
        let mut random = RandomGenerator::Legacy(LegacyRand::from_seed(1));
        assert_eq!(
            ChargeCursor::valid_movement_position(&level, pos, &mut random),
            Some(pos.up()),
        );
    }

    #[test]
    fn cursor_moves_onto_sculk_not_sensor() {
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos, Block::SCULK.default_state.id);
        level.set_id(pos.up(), Block::SCULK_SENSOR.default_state.id);
        level.set_id(pos.east(), Block::SCULK.default_state.id);
        let mut random = RandomGenerator::Legacy(LegacyRand::from_seed(1));
        assert_eq!(
            ChargeCursor::valid_movement_position(&level, pos, &mut random),
            Some(pos.east()),
        );
    }
}
