use std::sync::Arc;

use papokin_data::block_properties::{
    ChestLikeProperties, ChestType, CopperBulbLikeProperties, CopperGolemStatueLikeProperties,
    DoubleBlockHalf, EnumVariants, IronChainLikeProperties, LanternLikeProperties,
    MangroveRootsLikeProperties, OakDoorLikeProperties, OakFenceLikeProperties,
    OakTrapdoorLikeProperties, WhiteWoolSlabLikeProperties, WhiteWoolStairsLikeProperties,
};
use papokin_data::tag::Taggable;
use papokin_data::{Block, BlockId, BlockState, BlockStateId, Mirror, Rotation};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_world::world::BlockFlags;

use crate::block::blocks::doors::DoorBlock;
use crate::block::blocks::slabs::SlabBlock;
use crate::block::blocks::stairs::StairBlock;
use crate::block::blocks::trapdoor::TrapDoorBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, CanPlaceAtArgs, CanUpdateAtArgs,
    GetComparatorOutputArgs, GetStateForNeighborUpdateArgs, NormalUseArgs, OnNeighborUpdateArgs,
    OnPlaceArgs, OnStateReplacedArgs, PathComputationType, PlacedArgs, RandomTickArgs,
};
use crate::world::World;

/// 每个随机刻尝试降级的基础概率（约 5.69%）。
pub const BASE_DEGRADATION_CHANCE: f32 = 0.056_888_89;

/// 以曼哈顿距离度量的相邻铜块扫描距离。
pub const SCAN_DISTANCE: i32 = 4;

/// 可氧化铜方块的风化状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeatherState {
    Unaffected = 0,
    Exposed = 1,
    Weathered = 2,
    Oxidized = 3,
}

impl WeatherState {
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        *self as usize
    }

    #[must_use]
    pub const fn from_ordinal(ordinal: usize) -> Option<Self> {
        match ordinal {
            0 => Some(Self::Unaffected),
            1 => Some(Self::Exposed),
            2 => Some(Self::Weathered),
            3 => Some(Self::Oxidized),
            _ => None,
        }
    }

    #[must_use]
    pub const fn next(&self) -> Option<Self> {
        match self {
            Self::Unaffected => Some(Self::Exposed),
            Self::Exposed => Some(Self::Weathered),
            Self::Weathered => Some(Self::Oxidized),
            Self::Oxidized => None,
        }
    }

    #[must_use]
    pub const fn previous(&self) -> Option<Self> {
        match self {
            Self::Unaffected => None,
            Self::Exposed => Some(Self::Unaffected),
            Self::Weathered => Some(Self::Exposed),
            Self::Oxidized => Some(Self::Weathered),
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unaffected => "unaffected",
            Self::Exposed => "exposed",
            Self::Weathered => "weathered",
            Self::Oxidized => "oxidized",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "unaffected" => Some(Self::Unaffected),
            "exposed" => Some(Self::Exposed),
            "weathered" => Some(Self::Weathered),
            "oxidized" => Some(Self::Oxidized),
            _ => None,
        }
    }
}

/// 适用于基于随机刻随时间变化的方块的通用接口。
pub trait ChangeOverTimeBlock<T> {
    const SCAN_DISTANCE: i32 = SCAN_DISTANCE;
    const BASE_DEGRADATION_CHANCE: f32 = BASE_DEGRADATION_CHANCE;

    fn get_age(&self, block: &Block) -> Option<T>;
    fn get_chance_modifier(&self, age: T) -> f32;
    fn get_next(&self, block: &Block) -> Option<&'static Block>;
    fn get_previous(&self, block: &Block) -> Option<&'static Block>;
    fn get_first(&self, block: &Block) -> Option<&'static Block>;
}

/// 由所有会风化的铜方块实现的 trait。
pub trait WeatheringCopper: ChangeOverTimeBlock<WeatherState> {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

/// 全部 14 个铜方块族及其 4 个风化阶段（未风化、斑驳、风化、氧化）。
const COPPER_PROGRESSIONS: &[(&Block, &Block, &Block, &Block)] = &[
    (
        &Block::COPPER_BLOCK,
        &Block::EXPOSED_COPPER,
        &Block::WEATHERED_COPPER,
        &Block::OXIDIZED_COPPER,
    ),
    (
        &Block::CUT_COPPER,
        &Block::EXPOSED_CUT_COPPER,
        &Block::WEATHERED_CUT_COPPER,
        &Block::OXIDIZED_CUT_COPPER,
    ),
    (
        &Block::CHISELED_COPPER,
        &Block::EXPOSED_CHISELED_COPPER,
        &Block::WEATHERED_CHISELED_COPPER,
        &Block::OXIDIZED_CHISELED_COPPER,
    ),
    (
        &Block::CUT_COPPER_SLAB,
        &Block::EXPOSED_CUT_COPPER_SLAB,
        &Block::WEATHERED_CUT_COPPER_SLAB,
        &Block::OXIDIZED_CUT_COPPER_SLAB,
    ),
    (
        &Block::CUT_COPPER_STAIRS,
        &Block::EXPOSED_CUT_COPPER_STAIRS,
        &Block::WEATHERED_CUT_COPPER_STAIRS,
        &Block::OXIDIZED_CUT_COPPER_STAIRS,
    ),
    (
        &Block::COPPER_DOOR,
        &Block::EXPOSED_COPPER_DOOR,
        &Block::WEATHERED_COPPER_DOOR,
        &Block::OXIDIZED_COPPER_DOOR,
    ),
    (
        &Block::COPPER_TRAPDOOR,
        &Block::EXPOSED_COPPER_TRAPDOOR,
        &Block::WEATHERED_COPPER_TRAPDOOR,
        &Block::OXIDIZED_COPPER_TRAPDOOR,
    ),
    (
        &Block::COPPER_GRATE,
        &Block::EXPOSED_COPPER_GRATE,
        &Block::WEATHERED_COPPER_GRATE,
        &Block::OXIDIZED_COPPER_GRATE,
    ),
    (
        &Block::COPPER_BULB,
        &Block::EXPOSED_COPPER_BULB,
        &Block::WEATHERED_COPPER_BULB,
        &Block::OXIDIZED_COPPER_BULB,
    ),
    (
        &Block::COPPER_LANTERN,
        &Block::EXPOSED_COPPER_LANTERN,
        &Block::WEATHERED_COPPER_LANTERN,
        &Block::OXIDIZED_COPPER_LANTERN,
    ),
    (
        &Block::COPPER_CHEST,
        &Block::EXPOSED_COPPER_CHEST,
        &Block::WEATHERED_COPPER_CHEST,
        &Block::OXIDIZED_COPPER_CHEST,
    ),
    (
        &Block::COPPER_GOLEM_STATUE,
        &Block::EXPOSED_COPPER_GOLEM_STATUE,
        &Block::WEATHERED_COPPER_GOLEM_STATUE,
        &Block::OXIDIZED_COPPER_GOLEM_STATUE,
    ),
    (
        &Block::COPPER_BARS,
        &Block::EXPOSED_COPPER_BARS,
        &Block::WEATHERED_COPPER_BARS,
        &Block::OXIDIZED_COPPER_BARS,
    ),
    (
        &Block::COPPER_CHAIN,
        &Block::EXPOSED_COPPER_CHAIN,
        &Block::WEATHERED_COPPER_CHAIN,
        &Block::OXIDIZED_COPPER_CHAIN,
    ),
];

/// 返回氧化序列中的下一个方块；若已完全氧化或不是可风化铜方块，则返回 `None`。
#[must_use]
pub fn get_next(block: &Block) -> Option<&'static Block> {
    for &(unaffected, exposed, weathered, oxidized) in COPPER_PROGRESSIONS {
        if block == unaffected {
            return Some(exposed);
        }
        if block == exposed {
            return Some(weathered);
        }
        if block == weathered {
            return Some(oxidized);
        }
    }
    None
}

/// 返回氧化序列中上一个去氧化的方块；若未受氧化影响或不是可风化铜方块，则返回 `None`。
#[must_use]
pub fn get_previous(block: &Block) -> Option<&'static Block> {
    for &(unaffected, exposed, weathered, oxidized) in COPPER_PROGRESSIONS {
        if block == oxidized {
            return Some(weathered);
        }
        if block == weathered {
            return Some(exposed);
        }
        if block == exposed {
            return Some(unaffected);
        }
    }
    None
}

/// 返回此铜方块家族中首个未氧化的方块；若不是可风化铜方块，则返回 `None`。
#[must_use]
pub fn get_first(block: &Block) -> Option<&'static Block> {
    for &(unaffected, exposed, weathered, oxidized) in COPPER_PROGRESSIONS {
        if block == unaffected || block == exposed || block == weathered || block == oxidized {
            return Some(unaffected);
        }
    }
    None
}

///返回给定方块的 `WeatherState`；若它不是未涂蜡的风化铜方块，则返回 `None`。
#[must_use]
pub fn get_weather_state(block: &Block) -> Option<WeatherState> {
    for &(unaffected, exposed, weathered, oxidized) in COPPER_PROGRESSIONS {
        if block == unaffected {
            return Some(WeatherState::Unaffected);
        }
        if block == exposed {
            return Some(WeatherState::Exposed);
        }
        if block == weathered {
            return Some(WeatherState::Weathered);
        }
        if block == oxidized {
            return Some(WeatherState::Oxidized);
        }
    }
    None
}

/// 若方块还能进一步风化（存在下一状态），则返回 true。
#[must_use]
pub fn is_weathering(block: &Block) -> bool {
    get_next(block).is_some()
}

///返回给定天气状态的概率修正值（Unaffected 为 0.75，其他情况为 1.0）。
#[must_use]
pub const fn get_chance_modifier(state: WeatherState) -> f32 {
    match state {
        WeatherState::Unaffected => 0.75,
        WeatherState::Exposed | WeatherState::Weathered | WeatherState::Oxidized => 1.0,
    }
}

/// 将 `from_block` 的方块状态 ID 转换为 `to_block` 对应的状态 ID，并保留所有方块属性。
#[must_use]
pub fn with_properties_of(
    from_block: &Block,
    from_state_id: BlockStateId,
    to_block: &Block,
) -> BlockStateId {
    // 1. 无属性的完整方块
    if from_block == &Block::COPPER_BLOCK
        || from_block == &Block::EXPOSED_COPPER
        || from_block == &Block::WEATHERED_COPPER
        || from_block == &Block::OXIDIZED_COPPER
        || from_block == &Block::CUT_COPPER
        || from_block == &Block::EXPOSED_CUT_COPPER
        || from_block == &Block::WEATHERED_CUT_COPPER
        || from_block == &Block::OXIDIZED_CUT_COPPER
        || from_block == &Block::CHISELED_COPPER
        || from_block == &Block::EXPOSED_CHISELED_COPPER
        || from_block == &Block::WEATHERED_CHISELED_COPPER
        || from_block == &Block::OXIDIZED_CHISELED_COPPER
    {
        return to_block.default_state.id;
    }

    // 2. 楼梯
    if from_block.has_tag(&papokin_data::tag::Block::MINECRAFT_STAIRS) {
        let props = WhiteWoolStairsLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 3. 台阶
    if from_block.has_tag(&papokin_data::tag::Block::MINECRAFT_SLABS) {
        let props = WhiteWoolSlabLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 4. 门
    if from_block.has_tag(&papokin_data::tag::Block::MINECRAFT_DOORS) {
        let props = OakDoorLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 5. 活板门
    if from_block.has_tag(&papokin_data::tag::Block::MINECRAFT_TRAPDOORS) {
        let props = OakTrapdoorLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 6. 铜灯
    if from_block == &Block::COPPER_BULB
        || from_block == &Block::EXPOSED_COPPER_BULB
        || from_block == &Block::WEATHERED_COPPER_BULB
        || from_block == &Block::OXIDIZED_COPPER_BULB
    {
        let props = CopperBulbLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 7. 铜格栅
    if from_block == &Block::COPPER_GRATE
        || from_block == &Block::EXPOSED_COPPER_GRATE
        || from_block == &Block::WEATHERED_COPPER_GRATE
        || from_block == &Block::OXIDIZED_COPPER_GRATE
    {
        let props = MangroveRootsLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 8. 铜灯笼
    if from_block == &Block::COPPER_LANTERN
        || from_block == &Block::EXPOSED_COPPER_LANTERN
        || from_block == &Block::WEATHERED_COPPER_LANTERN
        || from_block == &Block::OXIDIZED_COPPER_LANTERN
    {
        let props = LanternLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 9. 铜箱子
    if from_block == &Block::COPPER_CHEST
        || from_block == &Block::EXPOSED_COPPER_CHEST
        || from_block == &Block::WEATHERED_COPPER_CHEST
        || from_block == &Block::OXIDIZED_COPPER_CHEST
    {
        let props = ChestLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 10. 铜傀儡雕像
    if from_block == &Block::COPPER_GOLEM_STATUE
        || from_block == &Block::EXPOSED_COPPER_GOLEM_STATUE
        || from_block == &Block::WEATHERED_COPPER_GOLEM_STATUE
        || from_block == &Block::OXIDIZED_COPPER_GOLEM_STATUE
    {
        let props = CopperGolemStatueLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 11. 铜栏杆
    if from_block == &Block::COPPER_BARS
        || from_block == &Block::EXPOSED_COPPER_BARS
        || from_block == &Block::WEATHERED_COPPER_BARS
        || from_block == &Block::OXIDIZED_COPPER_BARS
    {
        let props = OakFenceLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 12. 铜锁链
    if from_block == &Block::COPPER_CHAIN
        || from_block == &Block::EXPOSED_COPPER_CHAIN
        || from_block == &Block::WEATHERED_COPPER_CHAIN
        || from_block == &Block::OXIDIZED_COPPER_CHAIN
    {
        let props = IronChainLikeProperties::from_state_id(from_state_id);
        return props.to_state_id(to_block);
    }

    // 回退：使用相对状态索引
    let offset = from_state_id
        .as_u16()
        .saturating_sub(from_block.default_state.id.as_u16()) as usize;
    to_block
        .states
        .get(offset)
        .map_or(to_block.default_state.id, |s| s.id)
}

/// 返回可风化铜方块的下一阶段方块状态，并保留所有属性。
#[must_use]
pub fn get_next_state(current_block: &Block, state_id: BlockStateId) -> Option<BlockStateId> {
    let next_block = get_next(current_block)?;
    Some(with_properties_of(current_block, state_id, next_block))
}

/// 返回可风化铜方块的上一阶段方块状态，并保留所有属性。
#[must_use]
pub fn get_previous_state(current_block: &Block, state_id: BlockStateId) -> Option<BlockStateId> {
    let prev_block = get_previous(current_block)?;
    Some(with_properties_of(current_block, state_id, prev_block))
}

/// 返回此家族中首个（未受氧化影响的）方块状态，并保留所有属性。
#[must_use]
pub fn get_first_state(current_block: &Block, state_id: BlockStateId) -> BlockStateId {
    get_first(current_block).map_or(state_id, |first_block| {
        if first_block == current_block {
            state_id
        } else {
            with_properties_of(current_block, state_id, first_block)
        }
    })
}

/// 在曼哈顿距离 4 内扫描邻近的其他风化铜块。
///
///若任一相邻方块的氧化程度更低（这会完全抑制氧化），则返回 `None`。
///否则返回 `Some((same_age_count, higher_age_count))`。
#[must_use]
pub fn scan_neighbor_oxidation_levels(
    world: &World,
    center: &BlockPos,
    current_age: WeatherState,
) -> Option<(usize, usize)> {
    let mut same_age_count = 0;
    let mut higher_age_count = 0;

    for dx in -SCAN_DISTANCE..=SCAN_DISTANCE {
        for dy in -SCAN_DISTANCE..=SCAN_DISTANCE {
            for dz in -SCAN_DISTANCE..=SCAN_DISTANCE {
                let manhattan_dist = dx.abs() + dy.abs() + dz.abs();
                if manhattan_dist > SCAN_DISTANCE || manhattan_dist == 0 {
                    continue;
                }

                let neighbor_pos = BlockPos(Vector3::new(
                    center.0.x + dx,
                    center.0.y + dy,
                    center.0.z + dz,
                ));

                let neighbor_block = world.get_block(&neighbor_pos);
                if let Some(neighbor_age) = get_weather_state(neighbor_block) {
                    if neighbor_age < current_age {
                        // 被附近年龄更小的邻居抑制
                        return None;
                    }
                    if neighbor_age > current_age {
                        higher_age_count += 1;
                    } else {
                        same_age_count += 1;
                    }
                }
            }
        }
    }

    Some((same_age_count, higher_age_count))
}

/// 使用原版的概率公式，对会风化的铜方块执行一次随机刻风化尝试。
pub fn change_over_time(world: &Arc<World>, position: &BlockPos, block: &Block) {
    use rand::RngExt;

    // 1. 掷出基础退化概率（约 5.69%）
    if rand::rng().random::<f32>() >= BASE_DEGRADATION_CHANCE {
        return;
    }

    // 2. 必须是具有下一状态的可氧化方块
    let Some(current_age) = get_weather_state(block) else {
        return;
    };
    let Some(next_block) = get_next(block) else {
        return;
    };

    // 3. 扫描曼哈顿距离 4 内的相邻方块
    let Some((same_age_count, higher_age_count)) =
        scan_neighbor_oxidation_levels(world, position, current_age)
    else {
        return;
    };

    // 4. 计算概率：((higher + 1) / (higher + same + 1))^2 * chance_modifier
    let ratio = (higher_age_count + 1) as f32 / (higher_age_count + same_age_count + 1) as f32;
    let chance = ratio * ratio * get_chance_modifier(current_age);

    if rand::rng().random::<f32>() >= chance {
        return;
    }

    // 5. 应用状态变更
    let current_state_id = world.get_block_state_id(position);
    let new_state_id = with_properties_of(block, current_state_id, next_block);

    world.set_block_state(position, new_state_id, BlockFlags::NOTIFY_ALL);

    // 对多方块结构的特殊处理：
    // 门：若存在则更新上半部分
    if block.has_tag(&papokin_data::tag::Block::MINECRAFT_DOORS) {
        let door_props = OakDoorLikeProperties::from_state_id(current_state_id);
        if door_props.half == DoubleBlockHalf::Lower {
            let top_pos = position.up();
            let (top_block, top_state_id) = world.get_block_and_state_id(&top_pos);
            if top_block == block {
                let top_new_state_id = with_properties_of(top_block, top_state_id, next_block);
                world.set_block_state(&top_pos, top_new_state_id, BlockFlags::NOTIFY_ALL);
            }
        }
    }
    // 箱子：若为双联箱子则更新右侧的配对箱子
    else if block == &Block::COPPER_CHEST
        || block == &Block::EXPOSED_COPPER_CHEST
        || block == &Block::WEATHERED_COPPER_CHEST
    {
        let chest_props = ChestLikeProperties::from_state_id(current_state_id);
        if chest_props.r#type == ChestType::Left {
            let right_dir = chest_props.facing.rotate_clockwise();
            let right_pos = position.offset(right_dir.to_offset());
            let (right_block, right_state_id) = world.get_block_and_state_id(&right_pos);
            if right_block == block {
                let right_new_state_id =
                    with_properties_of(right_block, right_state_id, next_block);
                world.set_block_state(&right_pos, right_new_state_id, BlockFlags::NOTIFY_LISTENERS);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 方块实现
// ---------------------------------------------------------------------------

/// 处理标准的完全风化铜方块、格栅、栏杆、锁链、灯笼和雕像。
#[derive(Default)]
pub struct WeatheringCopperBlock;

impl ChangeOverTimeBlock<WeatherState> for WeatheringCopperBlock {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

impl WeatheringCopper for WeatheringCopperBlock {}

impl BlockMetadata for WeatheringCopperBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::COPPER_BLOCK,
            BlockId::EXPOSED_COPPER,
            BlockId::WEATHERED_COPPER,
            BlockId::OXIDIZED_COPPER,
            BlockId::CUT_COPPER,
            BlockId::EXPOSED_CUT_COPPER,
            BlockId::WEATHERED_CUT_COPPER,
            BlockId::OXIDIZED_CUT_COPPER,
            BlockId::CHISELED_COPPER,
            BlockId::EXPOSED_CHISELED_COPPER,
            BlockId::WEATHERED_CHISELED_COPPER,
            BlockId::OXIDIZED_CHISELED_COPPER,
            BlockId::COPPER_BARS,
            BlockId::EXPOSED_COPPER_BARS,
            BlockId::WEATHERED_COPPER_BARS,
            BlockId::OXIDIZED_COPPER_BARS,
            BlockId::COPPER_CHAIN,
            BlockId::EXPOSED_COPPER_CHAIN,
            BlockId::WEATHERED_COPPER_CHAIN,
            BlockId::OXIDIZED_COPPER_CHAIN,
            BlockId::COPPER_LANTERN,
            BlockId::EXPOSED_COPPER_LANTERN,
            BlockId::WEATHERED_COPPER_LANTERN,
            BlockId::OXIDIZED_COPPER_LANTERN,
            BlockId::COPPER_GOLEM_STATUE,
            BlockId::EXPOSED_COPPER_GOLEM_STATUE,
            BlockId::WEATHERED_COPPER_GOLEM_STATUE,
            BlockId::OXIDIZED_COPPER_GOLEM_STATUE,
        ]
        .into()
    }
}

impl BlockBehaviour for WeatheringCopperBlock {
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        change_over_time(args.world, args.position, args.block);
    }

    /// 只有雕像带有姿态，这里的其他铜方块均不读取任何内容。
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if !is_copper_golem_statue(args.block.id) {
            return None;
        }
        let props = CopperGolemStatueLikeProperties::from_state_id(args.state.id);
        // 原版读取姿态序号，从 1 开始。
        Some(props.copper_golem_pose.to_index() as u8 + 1)
    }
}

const fn is_copper_golem_statue(id: BlockId) -> bool {
    matches!(
        id,
        BlockId::COPPER_GOLEM_STATUE
            | BlockId::EXPOSED_COPPER_GOLEM_STATUE
            | BlockId::WEATHERED_COPPER_GOLEM_STATUE
            | BlockId::OXIDIZED_COPPER_GOLEM_STATUE
    )
}

/// 风化铜楼梯方块。
#[derive(Default)]
pub struct WeatheringCopperStairBlock;

impl ChangeOverTimeBlock<WeatherState> for WeatheringCopperStairBlock {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

impl WeatheringCopper for WeatheringCopperStairBlock {}

impl BlockMetadata for WeatheringCopperStairBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::CUT_COPPER_STAIRS,
            BlockId::EXPOSED_CUT_COPPER_STAIRS,
            BlockId::WEATHERED_CUT_COPPER_STAIRS,
            BlockId::OXIDIZED_CUT_COPPER_STAIRS,
        ]
        .into()
    }
}

impl BlockBehaviour for WeatheringCopperStairBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        StairBlock.on_place(args)
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        StairBlock.on_neighbor_update(args);
    }

    fn rotate(
        &self,
        block: &Block,
        state_id: BlockStateId,
        rotation: Rotation,
    ) -> &'static BlockState {
        StairBlock.rotate(block, state_id, rotation)
    }

    fn mirror(&self, block: &Block, state_id: BlockStateId, mirror: Mirror) -> &'static BlockState {
        StairBlock.mirror(block, state_id, mirror)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        change_over_time(args.world, args.position, args.block);
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        StairBlock.is_pathfindable(state, computation_type)
    }
}

/// 风化铜活板门方块。
#[derive(Default)]
pub struct WeatheringCopperTrapDoorBlock;

impl ChangeOverTimeBlock<WeatherState> for WeatheringCopperTrapDoorBlock {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

impl WeatheringCopper for WeatheringCopperTrapDoorBlock {}

impl BlockMetadata for WeatheringCopperTrapDoorBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::COPPER_TRAPDOOR,
            BlockId::EXPOSED_COPPER_TRAPDOOR,
            BlockId::WEATHERED_COPPER_TRAPDOOR,
            BlockId::OXIDIZED_COPPER_TRAPDOOR,
        ]
        .into()
    }
}

impl BlockBehaviour for WeatheringCopperTrapDoorBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        TrapDoorBlock.on_place(args)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        TrapDoorBlock.normal_use(args)
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        TrapDoorBlock.on_neighbor_update(args);
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        change_over_time(args.world, args.position, args.block);
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        TrapDoorBlock.is_pathfindable(state, computation_type)
    }
}

/// 风化铜台阶方块。
#[derive(Default)]
pub struct WeatheringCopperSlabBlock;

impl ChangeOverTimeBlock<WeatherState> for WeatheringCopperSlabBlock {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

impl WeatheringCopper for WeatheringCopperSlabBlock {}

impl BlockMetadata for WeatheringCopperSlabBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::CUT_COPPER_SLAB,
            BlockId::EXPOSED_CUT_COPPER_SLAB,
            BlockId::WEATHERED_CUT_COPPER_SLAB,
            BlockId::OXIDIZED_CUT_COPPER_SLAB,
        ]
        .into()
    }
}

impl BlockBehaviour for WeatheringCopperSlabBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        SlabBlock.on_place(args)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        SlabBlock.can_update_at(args)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        change_over_time(args.world, args.position, args.block);
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        SlabBlock.is_pathfindable(state, computation_type)
    }
}

/// 风化铜门方块。
#[derive(Default)]
pub struct WeatheringCopperDoorBlock;

impl ChangeOverTimeBlock<WeatherState> for WeatheringCopperDoorBlock {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

impl WeatheringCopper for WeatheringCopperDoorBlock {}

impl BlockMetadata for WeatheringCopperDoorBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::COPPER_DOOR,
            BlockId::EXPOSED_COPPER_DOOR,
            BlockId::WEATHERED_COPPER_DOOR,
            BlockId::OXIDIZED_COPPER_DOOR,
        ]
        .into()
    }
}

impl BlockBehaviour for WeatheringCopperDoorBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        DoorBlock.on_place(args)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        DoorBlock.normal_use(args)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        DoorBlock.can_place_at(args)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        DoorBlock.placed(args);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        DoorBlock.broken(args);
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        DoorBlock.on_neighbor_update(args);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        DoorBlock.get_state_for_neighbor_update(args)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        DoorBlock.on_state_replaced(args);
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let door_props = OakDoorLikeProperties::from_state_id(state_id);
        if door_props.half == DoubleBlockHalf::Lower {
            change_over_time(args.world, args.position, args.block);
        }
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        DoorBlock.is_pathfindable(state, computation_type)
    }
}

/// 风化铜格栅方块。
#[derive(Default)]
pub struct WeatheringCopperGrateBlock;

impl ChangeOverTimeBlock<WeatherState> for WeatheringCopperGrateBlock {
    fn get_age(&self, block: &Block) -> Option<WeatherState> {
        get_weather_state(block)
    }

    fn get_chance_modifier(&self, age: WeatherState) -> f32 {
        get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        get_first(block)
    }
}

impl WeatheringCopper for WeatheringCopperGrateBlock {}

impl BlockMetadata for WeatheringCopperGrateBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::COPPER_GRATE,
            BlockId::EXPOSED_COPPER_GRATE,
            BlockId::WEATHERED_COPPER_GRATE,
            BlockId::OXIDIZED_COPPER_GRATE,
        ]
        .into()
    }
}

impl BlockBehaviour for WeatheringCopperGrateBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = MangroveRootsLikeProperties::default(args.block);
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        change_over_time(args.world, args.position, args.block);
    }
}
