use papokin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::block_properties::{BlockProperties, COLLISION_SHAPES, NoteblockInstrument};
use crate::{Block, BlockDirection, BlockId};

/// 表示方块的某个特定状态，包括其属性和物理行为。
///
/// 单个 `Block`（例如漏斗）可以拥有多个 `BlockState`（例如朝向北、
/// 朝南，或被充能）。此结构体针对高速查找进行了优化，用于
/// 物理和光照计算。
#[derive(Debug)]
pub struct BlockState {
    /// 用于网络序列化和区块存储的全局调色板 ID。
    pub id: BlockStateId,
    /// 表示布尔或枚举属性的位标志（如 `waterlogged`、`lit`、`facing`）。
    pub state_flags: u16,
    /// 缓存 6 个面各自的标志，以加速环境光遮蔽与面剔除。
    pub side_flags: u8,
    /// 将此方块放在音符盒下方时产生的音符盒乐器。
    pub instrument: NoteblockInstrument,
    /// 此方块发出的光照等级，范围从 0 到 15。
    pub luminance: u8,
    /// 定义方块对被活塞推动或拉动的反应。
    pub piston_behavior: PistonBehavior,
    /// 如有必要，覆盖此特定状态的基础方块硬度。
    pub hardness: f32,
    /// 实体物理碰撞所用全局体素形状注册表的索引。
    pub collision_shapes: &'static [u16],
    /// 选择高亮框所用全局体素形状注册表的索引。
    pub outline_shapes: &'static [u16],
    /// 光线穿过此方块时的衰减量（透明为 0，不透明为 15）。
    pub opacity: u8,
    /// 与此状态关联的方块实体 ID。
    /// 若方块不持有 NBT 数据，则设为 `u16::MAX`。
    pub block_entity_type: u16,
}

/// 用于确保从外部来源解析出的 `BlockStateIds` 有效性的辅助结构体。
/// 每个 [`BlockStateId`] 都保证对应一个有效的 [`BlockState`]。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BlockStateId(u16);

#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PistonBehavior {
    Normal,
    Destroy,
    Block,
    Ignore,
    PushOnly,
}

impl PartialEq<BlockStateId> for BlockState {
    fn eq(&self, other: &BlockStateId) -> bool {
        self.id == *other
    }
}

impl PartialEq<BlockState> for BlockStateId {
    fn eq(&self, other: &BlockState) -> bool {
        *self == other.id
    }
}

impl PartialEq for BlockState {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for BlockState {}

impl BlockState {
    #[must_use]
    pub const fn is_air(&self) -> bool {
        self.state_flags & IS_AIR != 0
    }

    #[must_use]
    pub const fn burnable(&self) -> bool {
        self.state_flags & BURNABLE != 0
    }

    #[must_use]
    pub const fn tool_required(&self) -> bool {
        self.state_flags & TOOL_REQUIRED != 0
    }

    #[must_use]
    pub const fn sided_transparency(&self) -> bool {
        self.state_flags & SIDED_TRANSPARENCY != 0
    }

    #[must_use]
    pub const fn replaceable(&self) -> bool {
        self.state_flags & REPLACEABLE != 0
    }

    #[must_use]
    pub const fn is_liquid(&self) -> bool {
        self.state_flags & IS_LIQUID != 0
    }

    /// 返回方块是否为实心的旧版值。
    #[must_use]
    pub const fn is_solid(&self) -> bool {
        self.state_flags & IS_SOLID != 0
    }

    #[must_use]
    pub const fn is_full_cube(&self) -> bool {
        self.state_flags & IS_FULL_CUBE != 0
    }

    /// 返回方块是否为实心。
    /// 固体方块会传导红石并阻挡红石线。
    /// 在 Java 版中，非固体方块不允许其上方的红石线向下传递信号。
    #[must_use]
    pub const fn is_solid_block(&self) -> bool {
        self.state_flags & IS_SOLID_BLOCK != 0
    }

    #[must_use]
    pub const fn has_random_ticks(&self) -> bool {
        self.state_flags & HAS_RANDOM_TICKS != 0
    }

    /// 返回此方块状态是否为完整的不透明立方体（Java 中的 `isSolidRender()`）。
    #[must_use]
    pub const fn is_solid_render(&self) -> bool {
        self.state_flags & IS_SOLID_RENDER != 0
    }

    /// 返回此方块状态是否可遮挡其他方块（Java 中的 `canOcclude`）。
    #[must_use]
    pub const fn can_occlude(&self) -> bool {
        self.state_flags & CAN_OCCLUDE != 0
    }

    /// 返回此方块是否为比较器产生模拟输出信号（Java 中的 `hasAnalogOutputSignal()`）。
    #[must_use]
    pub const fn has_analog_output_signal(&self) -> bool {
        self.state_flags & HAS_ANALOG_OUTPUT_SIGNAL != 0
    }

    ///相当于 Java 的 `isFaceSturdy()`！
    #[must_use]
    pub const fn is_side_solid(&self, side: BlockDirection) -> bool {
        match side {
            BlockDirection::Down => self.side_flags & DOWN_SIDE_SOLID != 0,
            BlockDirection::Up => self.side_flags & UP_SIDE_SOLID != 0,
            BlockDirection::North => self.side_flags & NORTH_SIDE_SOLID != 0,
            BlockDirection::South => self.side_flags & SOUTH_SIDE_SOLID != 0,
            BlockDirection::West => self.side_flags & WEST_SIDE_SOLID != 0,
            BlockDirection::East => self.side_flags & EAST_SIDE_SOLID != 0,
        }
    }

    ///相当于 Java 的 isSideSolid(..., Direction.UP, SideShapeType.CENTER)！
    ///仅对 UP 和 DOWN 面有效
    #[must_use]
    pub const fn is_center_solid(&self, side: BlockDirection) -> bool {
        match side {
            BlockDirection::Down => self.side_flags & DOWN_CENTER_SOLID != 0,
            BlockDirection::Up => self.side_flags & UP_CENTER_SOLID != 0,
            _ => false,
        }
    }

    #[must_use]
    pub fn is_waterlogged(&self) -> bool {
        self.id.is_waterlogged()
    }

    /// 转为原版的方块状态字符串（1.20.3+ 的 NBT 存储形式），
    /// 例如 `minecraft:oak_stairs[facing=east,waterlogged=false]`；
    /// 无属性的方块输出裸名称，如 `minecraft:redstone_block`。
    ///
    /// 活塞方块实体把被移动方块的该字符串写入 `blockState` 字段，
    /// 客户端靠它渲染移动动画并把方块落到最终位置——缺失时客户端
    /// 会当作空气，动画结束把目标位置覆盖成空气（方块“消失”）。
    #[must_use]
    pub fn to_state_string(&self) -> String {
        let block = Block::from_state_id(self.id);
        let mut result = format!("minecraft:{}", block.name);
        let Some(props) = block.properties(self.id) else {
            return result;
        };
        let entries = props.to_props();
        if entries.is_empty() {
            return result;
        }
        result.push('[');
        result.push_str(
            &entries
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        result.push(']');
        result
    }

    /// 解析 [`Self::to_state_string`] 生成的方块状态字符串（逆操作）。
    /// 未指定的属性取方块默认值，无法识别的属性键被忽略。
    #[must_use]
    pub fn from_state_string(string: &str) -> Option<&'static Self> {
        let raw = string.strip_prefix("minecraft:").unwrap_or(string);
        let (name, properties) = match raw.split_once('[') {
            Some((name, rest)) => (name, rest.strip_suffix(']')?),
            None => (raw, ""),
        };
        let block = Block::from_registry_key(name)?;
        if properties.is_empty() {
            return Some(block.default_state);
        }
        let entries: Vec<(&str, &str)> = properties
            .split(',')
            .map(|pair| pair.split_once('='))
            .collect::<Option<_>>()?;
        let state_id = block.from_properties(&entries).to_state_id(block);
        Some(state_id.to_state())
    }

    /// 生成一个除含水属性外与 `self` 完全相同的新状态
    /// 被设置为 `value`。如果方块类型不支持含水或
    /// 该状态已将 waterlogged 设为 `value`，则返回 `None`。
    #[must_use]
    pub fn set_waterlogged(&self, value: bool) -> Option<&'static BlockState> {
        self.id
            .to_block()
            .set_waterlogged(self.id, value)
            .map(BlockStateId::to_state)
    }

    pub fn get_block_collision_shapes(&self) -> impl Iterator<Item = BoundingBox> + '_ {
        self.collision_shapes
            .iter()
            .map(|&id| COLLISION_SHAPES[id as usize])
    }

    ///返回方块局部的碰撞形状，并应用原版基于坐标推导的偏移。
    pub fn get_block_collision_shapes_at(
        &self,
        pos: &BlockPos,
    ) -> impl Iterator<Item = BoundingBox> + '_ {
        let offset = Block::from_state_id(self.id).shape_offset_delta(pos);
        self.get_block_collision_shapes()
            .map(move |shape| shape.shift(offset))
    }

    pub fn get_block_outline_shapes(&self) -> impl Iterator<Item = BoundingBox> + '_ {
        let base_shapes = self
            .outline_shapes
            .iter()
            .map(|&id| COLLISION_SHAPES[id as usize]);

        let water_shape = self
            .is_waterlogged()
            .then(|| BoundingBox::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.875, 1.0)));

        base_shapes.chain(water_shape)
    }

    ///返回方块局部的轮廓形状，并应用原版基于坐标推导的偏移。
    pub fn get_block_outline_shapes_at(
        &self,
        pos: &BlockPos,
    ) -> impl Iterator<Item = BoundingBox> + '_ {
        let offset = Block::from_state_id(self.id).shape_offset_delta(pos);
        let base_shapes = self
            .outline_shapes
            .iter()
            .map(move |&id| COLLISION_SHAPES[id as usize].shift(offset));

        let water_shape = self
            .is_waterlogged()
            .then(|| BoundingBox::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.875, 1.0)));

        base_shapes.chain(water_shape)
    }

    #[must_use]
    pub fn rotate(&self, rotation: crate::block_rotation::Rotation) -> &'static Self {
        Block::from_state_id(self.id).rotate(self.id, rotation)
    }

    #[must_use]
    pub fn mirror(&self, mirror: crate::block_rotation::Mirror) -> &'static Self {
        Block::from_state_id(self.id).mirror(self.id, mirror)
    }
}

impl BlockStateId {
    // 取决于生成的 impl：
    // pub(crate) const STATE_COUNT: u16;

    /// 所有已注册方块状态的总数。
    pub const COUNT: u16 = Self::STATE_COUNT;

    // SAFETY: 绝不允许出现 self.0 >= BlockStateId::STATE_COUNT 的 BlockStateId

    #[inline]
    #[must_use]
    pub const fn new(inner: u16) -> Option<Self> {
        if inner < Self::STATE_COUNT {
            return Some(Self(inner));
        }
        None
    }

    #[inline]
    #[must_use]
    pub const fn new_or_air(inner: u16) -> Self {
        if inner < Self::STATE_COUNT {
            return Self(inner);
        }
        Self::AIR
    }

    #[inline(always)]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn to_state(self) -> &'static BlockState {
        BlockState::from_id(self)
    }

    #[inline]
    #[must_use]
    pub const fn to_block_id(self) -> BlockId {
        BlockId::from_state_id(self)
    }

    #[inline]
    #[must_use]
    pub const fn to_block(self) -> &'static Block {
        Block::from_state_id(self)
    }

    #[inline]
    #[must_use]
    pub const fn is_solid_render(self) -> bool {
        self.to_state().is_solid_render()
    }

    #[inline]
    #[must_use]
    pub const fn can_occlude(self) -> bool {
        self.to_state().can_occlude()
    }

    #[inline]
    #[must_use]
    pub const fn has_analog_output_signal(self) -> bool {
        self.to_state().has_analog_output_signal()
    }

    #[inline]
    #[must_use]
    pub fn is_waterlogged(self) -> bool {
        self.to_block().is_waterlogged(self)
    }

    #[inline]
    #[must_use]
    pub fn rotate(self, rotation: crate::block_rotation::Rotation) -> &'static BlockState {
        Block::from_state_id(self).rotate(self, rotation)
    }

    #[inline]
    #[must_use]
    pub fn mirror(self, mirror: crate::block_rotation::Mirror) -> &'static BlockState {
        Block::from_state_id(self).mirror(self, mirror)
    }
}

impl Default for BlockStateId {
    #[inline]
    fn default() -> Self {
        Self::AIR
    }
}

impl std::fmt::Display for BlockStateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BlockStateId({} = \"{}\")",
            self.0,
            Block::from_state_id(*self).name
        )
    }
}

//这是按正确顺序排列的 state_props 布局
// state_flags
const IS_AIR: u16 = 1 << 0;
const BURNABLE: u16 = 1 << 1;
const TOOL_REQUIRED: u16 = 1 << 2;
const SIDED_TRANSPARENCY: u16 = 1 << 3;
const REPLACEABLE: u16 = 1 << 4;
const IS_LIQUID: u16 = 1 << 5;
const IS_SOLID: u16 = 1 << 6;
const IS_FULL_CUBE: u16 = 1 << 7;
const IS_SOLID_BLOCK: u16 = 1 << 8;
const HAS_RANDOM_TICKS: u16 = 1 << 9;
const IS_SOLID_RENDER: u16 = 1 << 10;
const CAN_OCCLUDE: u16 = 1 << 11;
const HAS_ANALOG_OUTPUT_SIGNAL: u16 = 1 << 12;

// side_flags
const DOWN_SIDE_SOLID: u8 = 1 << 0;
const UP_SIDE_SOLID: u8 = 1 << 1;
const NORTH_SIDE_SOLID: u8 = 1 << 2;
const SOUTH_SIDE_SOLID: u8 = 1 << 3;
const WEST_SIDE_SOLID: u8 = 1 << 4;
const EAST_SIDE_SOLID: u8 = 1 << 5;
const DOWN_CENTER_SOLID: u8 = 1 << 6;
const UP_CENTER_SOLID: u8 = 1 << 7;

#[cfg(test)]
mod tests {
    use crate::{
        Block, BlockState, BlockStateId, block_state_remap::remap_block_state_for_version,
    };
    use papokin_util::{math::position::BlockPos, version::JavaMinecraftVersion};

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
    }

    #[test]
    fn bamboo_collision_shape_uses_its_world_position() {
        let state = Block::BAMBOO.default_state;
        let origin_shape = state
            .get_block_collision_shapes_at(&BlockPos::new(0, 64, 0))
            .next()
            .unwrap();
        let shifted_shape = state
            .get_block_collision_shapes_at(&BlockPos::new(-18, 64, -7))
            .next()
            .unwrap();

        assert_close(origin_shape.min.x, 0.15625);
        assert_close(origin_shape.max.x, 0.34375);
        assert_close(shifted_shape.min.x, 0.65625);
        assert_close(shifted_shape.max.x, 0.84375);
        assert_close(shifted_shape.min.z, 0.65625);
        assert_close(shifted_shape.max.z, 0.84375);
    }

    #[test]
    fn supported_client_versions_keep_offset_collisions_mapped() {
        let versions = [
            JavaMinecraftVersion::V_1_20_5,
            JavaMinecraftVersion::V_1_21,
            JavaMinecraftVersion::V_1_21_2,
            JavaMinecraftVersion::V_1_21_4,
            JavaMinecraftVersion::V_1_21_5,
            JavaMinecraftVersion::V_1_21_6,
            JavaMinecraftVersion::V_1_21_7,
            JavaMinecraftVersion::V_1_21_9,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_1,
            JavaMinecraftVersion::V_26_2,
            JavaMinecraftVersion::V_26_3,
        ];

        for version in versions {
            for block in [Block::BAMBOO, Block::POINTED_DRIPSTONE] {
                assert_ne!(
                    remap_block_state_for_version(block.default_state.id.as_u16(), version),
                    BlockStateId::AIR.as_u16(),
                    "{} mapped to air for {version}",
                    block.name
                );
            }
        }
    }

    /// 无属性方块输出裸名称；这是活塞方块实体 `blockState` 字段的
    /// 线上格式（1.20.3+），客户端渲染移动中的方块依赖它。
    #[test]
    fn propertyless_block_states_serialize_to_bare_names() {
        assert_eq!(
            Block::REDSTONE_BLOCK.default_state.to_state_string(),
            "minecraft:redstone_block"
        );
        assert_eq!(
            BlockState::from_state_string("minecraft:redstone_block"),
            Some(Block::REDSTONE_BLOCK.default_state)
        );
    }

    /// 全量往返：每个方块状态的字符串形式必须能无损解析回原状态。
    #[test]
    fn all_block_state_strings_round_trip() {
        for raw in 0..BlockStateId::COUNT {
            let Some(state_id) = BlockStateId::new(raw) else {
                continue;
            };
            let state = state_id.to_state();
            let parsed = BlockState::from_state_string(&state.to_state_string());
            assert_eq!(
                parsed.map(|parsed| parsed.id),
                Some(state.id),
                "状态 id {raw}（{}）往返失败",
                Block::from_state_id(state.id).name
            );
        }
    }
}
