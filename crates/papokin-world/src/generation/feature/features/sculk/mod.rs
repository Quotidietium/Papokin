//! 幽匿斑块生成：电荷光标蔓延幽匿并放置
//! 感测体 / 尖啸体的生长。
//!
//! 组织为三个子模块：
//! - [`spreader`] — 光标移动、电荷衰减、合并。
//! - [`vein`]     — 借助支撑面进行的多面（幽匿脉络）蔓延。
//! - [`growth`]   — 幽匿感测体 / 幽匿尖啸体的放置规则。

use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::BlockId;
use papokin_data::BlockState;
use papokin_data::BlockStateId;
use papokin_data::block_properties::GlowLichenLikeProperties;
use papokin_data::block_properties::is_air;
use papokin_data::fluid::Fluid;
use papokin_data::tag::Block::MINECRAFT_SCULK_REPLACEABLE;
use papokin_data::tag::Block::MINECRAFT_SCULK_REPLACEABLE_WORLD_GEN;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub mod growth;
pub mod spreader;
pub mod vein;

/// 一个方块的 18 个非角相邻位置。
///
/// 3×3×3 立方体中至少有一个坐标轴为零的每个偏移
/// (即不是角落) 且也不是中心本身。
pub const NON_CORNER_NEIGHBOURS: [Vector3<i32>; 18] = [
    // 面相邻 — 6
    Vector3::new(-1, 0, 0),
    Vector3::new(1, 0, 0),
    Vector3::new(0, -1, 0),
    Vector3::new(0, 1, 0),
    Vector3::new(0, 0, -1),
    Vector3::new(0, 0, 1),
    // 边相邻（同一 Y 平面）— 4
    Vector3::new(-1, 0, -1),
    Vector3::new(-1, 0, 1),
    Vector3::new(1, 0, -1),
    Vector3::new(1, 0, 1),
    // 边相邻（X 方向垂直）— 4
    Vector3::new(-1, -1, 0),
    Vector3::new(-1, 1, 0),
    Vector3::new(1, -1, 0),
    Vector3::new(1, 1, 0),
    // 边相邻（Z 方向垂直）— 4
    Vector3::new(0, -1, -1),
    Vector3::new(0, -1, 1),
    Vector3::new(0, 1, -1),
    Vector3::new(0, 1, 1),
];

/// 对幽匿蔓延器所读取和写入的层级的抽象。
///
/// 通过 blanket impl 为 `T: GenerationCache`（地形生成之后）提供实现，
/// 以及通过 [`ProtoChunkSculkView`] 进行的原型区块生成。
pub trait SculkLevel {
    ///返回 `pos` 处的方块状态；若该位置未加载，则返回 `None`
    /// 不可写（超出正在生成的区块范围）。`None` 被视为
    /// 传播器视为固体障碍——游标无法穿过它。
    fn sculk_get(&self, pos: BlockPos) -> Option<BlockStateId>;

    /// 设置 `pos` 处的方块状态。若该位置不可写则为空操作。
    fn sculk_set(&mut self, pos: BlockPos, state: &'static BlockState);

    /// 该位置是否完全为空（空气或虚空）。
    fn sculk_is_air(&self, pos: BlockPos) -> bool;

    /// 该位置是否含水（水源）。
    fn sculk_is_water_source(&self, pos: BlockPos) -> bool;

    /// 该位置是否含水（任意水的流体状态）。
    fn sculk_is_water(&self, pos: BlockPos) -> bool;

    /// `pos` 处方块在给定方向上的面是否
    /// 坚固（等价于 Java 的 `isFaceSturdy`）。
    fn sculk_is_face_sturdy(&self, pos: BlockPos, face: BlockDirection) -> bool;

    /// `pos` 处的方块是否为完整立方体（碰撞箱为完整方块，
    /// 由 `canSpreadFrom` 使用）。
    fn sculk_is_full_cube(&self, pos: BlockPos) -> bool;
}

///若该方块 ID 在蔓延方面表现得像幽匿块，则返回 `true`。
///
/// 只有 `sculk` 和 `sculk_vein` 符合条件。幽匿感测体、幽匿尖啸体和幽匿催发体
/// 是普通方块，并在游标访问时使用默认行为
/// 它们。
#[must_use]
pub const fn is_sculk_behaviour(id: BlockId) -> bool {
    matches!(id, BlockId::SCULK | BlockId::SCULK_VEIN)
}

///若该方块 ID 带有 `minecraft:sculk_replaceable` 标签，则返回 `true`。
#[must_use]
pub fn is_sculk_replaceable(id: BlockId) -> bool {
    id.has_tag(MINECRAFT_SCULK_REPLACEABLE)
}

///若该方块状态是含水的幽匿脉络：其中持有水，则返回 `true`
/// 水流体，尽管它本身不是水方块。
#[must_use]
fn is_waterlogged_vein(state: BlockStateId) -> bool {
    state.to_block_id() == BlockId::SCULK_VEIN
        && GlowLichenLikeProperties::from_state_id(state).waterlogged
}

///若该方块 ID 被打上了标签，则返回 `true`
/// `minecraft:sculk_replaceable_world_gen`。
#[must_use]
pub fn is_sculk_replaceable_world_gen(id: BlockId) -> bool {
    id.has_tag(MINECRAFT_SCULK_REPLACEABLE_WORLD_GEN)
}

/// 检查幽匿斑块能否在给定位置生成。
pub fn can_spread_from(level: &dyn SculkLevel, pos: BlockPos) -> bool {
    let Some(state) = level.sculk_get(pos) else {
        return false;
    };
    let block_id = state.to_block_id();
    if is_sculk_behaviour(block_id) {
        return true;
    }
    // 只有空气或实际存有水源的水方块才符合条件；
    // 而含水的非水方块则不会。
    if !(level.sculk_is_air(pos) || block_id == BlockId::WATER && level.sculk_is_water_source(pos))
    {
        return false;
    }
    // 位置是空气或水源——需要邻居为完整方块。
    BlockDirection::all()
        .into_iter()
        .any(|dir| level.sculk_is_full_cube(pos.offset(dir.to_offset())))
}

/// 构建额外稀有生长处理阶段所用的幽匿尖啸体方块状态
/// (世界生成时会设置 `can_summon`)。
#[must_use]
pub fn shrieker_state(can_summon: bool) -> &'static BlockState {
    let mut properties = papokin_data::block_properties::SculkShriekerLikeProperties::default(
        &Block::SCULK_SHRIEKER,
    );
    properties.r#can_summon = can_summon;
    BlockState::from_id(properties.to_state_id(&Block::SCULK_SHRIEKER))
}

/// 便捷方法：解析给定位置的方块 ID，无法解析时默认为空气
/// 不可写入。
pub fn sculk_block_id(level: &dyn SculkLevel, pos: BlockPos) -> BlockId {
    level
        .sculk_get(pos)
        .map_or(BlockId::AIR, BlockStateId::to_block_id)
}

// 对任何实现 GenerationCache 的类型提供统一实现（地形生成后场景）。

use crate::generation::proto_chunk::GenerationCache;

/// 为每个实现了该 trait 的类型提供的 `SculkLevel` 全面实现
/// `GenerationCache`。这涵盖了地形生成后阶段所使用的 `Cache`
/// 地物生成。
impl<T: GenerationCache> SculkLevel for T {
    fn sculk_get(&self, pos: BlockPos) -> Option<BlockStateId> {
        Some(GenerationCache::get_block_state(self, &pos.0))
    }

    fn sculk_set(&mut self, pos: BlockPos, state: &'static BlockState) {
        GenerationCache::set_block_state(self, &pos.0, state);
    }

    fn sculk_is_air(&self, pos: BlockPos) -> bool {
        GenerationCache::is_air(self, &pos.0)
    }

    fn sculk_is_water_source(&self, pos: BlockPos) -> bool {
        let (fluid, fluid_state) = GenerationCache::get_fluid_and_fluid_state(self, &pos.0);
        fluid == Fluid::WATER && fluid_state.is_source
    }

    fn sculk_is_water(&self, pos: BlockPos) -> bool {
        let (fluid, _fluid_state) = GenerationCache::get_fluid_and_fluid_state(self, &pos.0);
        fluid == Fluid::WATER
    }

    fn sculk_is_face_sturdy(&self, pos: BlockPos, face: BlockDirection) -> bool {
        GenerationCache::get_block_state(self, &pos.0)
            .to_state()
            .is_side_solid(face)
    }

    fn sculk_is_full_cube(&self, pos: BlockPos) -> bool {
        GenerationCache::get_block_state(self, &pos.0)
            .to_state()
            .is_full_cube()
    }
}

// 用于世界生成（感知边界）场景的 ProtoChunk 包装器。

use crate::ProtoChunk;

/// 一个带边界检查的 [`ProtoChunk`] 视图，实现了 [`SculkLevel`]。
///
/// 与泛化的 `GenerationCache` 实现（会读取……中的任意区块）不同，
/// 缓存），此包装器对 proto 之外的位置返回 `None`
/// 区块 XZ 边界或高度范围之外，并正确地将它们建模为实心
/// 对传播者而言的障碍物。
pub struct ProtoChunkSculkView<'a> {
    chunk: &'a mut ProtoChunk,
}

impl<'a> ProtoChunkSculkView<'a> {
    /// 基于所提供的……创建一个排他且感知边界的视图
    /// [`ProtoChunk`]。越界位置的读取会返回 `None`
    /// 且对写入是空操作，将它们建模为固体障碍物，供
    /// 传播器。
    pub const fn new(chunk: &'a mut ProtoChunk) -> Self {
        Self { chunk }
    }

    const fn in_bounds(&self, pos: BlockPos) -> bool {
        if pos.0.x >> 4 != self.chunk.x || pos.0.z >> 4 != self.chunk.z {
            return false;
        }
        let local_y = pos.0.y - self.chunk.bottom_y() as i32;
        local_y >= 0 && local_y < self.chunk.height() as i32
    }
}

impl SculkLevel for ProtoChunkSculkView<'_> {
    fn sculk_get(&self, pos: BlockPos) -> Option<BlockStateId> {
        if !self.in_bounds(pos) {
            return None;
        }
        let local_x = pos.0.x & 15;
        let local_y = pos.0.y - self.chunk.bottom_y() as i32;
        let local_z = pos.0.z & 15;
        Some(self.chunk.get_block_state_raw(local_x, local_y, local_z))
    }

    fn sculk_set(&mut self, pos: BlockPos, state: &'static BlockState) {
        if !self.in_bounds(pos) {
            return;
        }
        let local_x = pos.0.x & 15;
        let local_z = pos.0.z & 15;
        self.chunk.set_block_state(local_x, pos.0.y, local_z, state);
    }

    fn sculk_is_air(&self, pos: BlockPos) -> bool {
        self.sculk_get(pos).is_none_or(is_air)
    }

    fn sculk_is_water_source(&self, pos: BlockPos) -> bool {
        // 原型区块没有流体模拟；水以方块形式存储
        // 状态，其中水方块（按惯例）是水源，因此
        // WATER 方块状态被视为水源。
        self.sculk_get(pos)
            .is_some_and(|s| s.to_block_id() == BlockId::WATER)
    }

    fn sculk_is_water(&self, pos: BlockPos) -> bool {
        self.sculk_get(pos)
            .is_some_and(|s| s.to_block_id() == BlockId::WATER || is_waterlogged_vein(s))
    }

    fn sculk_is_face_sturdy(&self, pos: BlockPos, face: BlockDirection) -> bool {
        self.sculk_get(pos)
            .is_some_and(|s| s.to_state().is_side_solid(face))
    }

    fn sculk_is_full_cube(&self, pos: BlockPos) -> bool {
        self.sculk_get(pos)
            .is_some_and(|s| s.to_state().is_full_cube())
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use std::collections::HashMap;

    /// 供 sculk（幽匿）单元测试使用的内存版 [`SculkLevel`]
    /// 子模块。与原型区块视图保持一致：水是从
    /// 方块状态，缺失的位置读取为空气。
    pub struct MockSculkLevel {
        pub(crate) blocks: HashMap<BlockPos, BlockStateId>,
    }

    impl MockSculkLevel {
        pub(crate) fn new() -> Self {
            Self {
                blocks: HashMap::new(),
            }
        }

        pub(crate) fn set_id(&mut self, pos: BlockPos, id: BlockStateId) {
            self.blocks.insert(pos, id);
        }
    }

    impl SculkLevel for MockSculkLevel {
        fn sculk_get(&self, pos: BlockPos) -> Option<BlockStateId> {
            self.blocks.get(&pos).copied()
        }

        fn sculk_set(&mut self, pos: BlockPos, state: &'static BlockState) {
            self.blocks.insert(pos, state.id);
        }

        fn sculk_is_air(&self, pos: BlockPos) -> bool {
            self.sculk_get(pos).is_none_or(|s| s.to_state().is_air())
        }

        fn sculk_is_water_source(&self, pos: BlockPos) -> bool {
            self.sculk_get(pos)
                .is_some_and(|s| s.to_block_id() == BlockId::WATER)
        }

        fn sculk_is_water(&self, pos: BlockPos) -> bool {
            self.sculk_get(pos)
                .is_some_and(|s| s.to_block_id() == BlockId::WATER || is_waterlogged_vein(s))
        }

        fn sculk_is_face_sturdy(&self, pos: BlockPos, face: BlockDirection) -> bool {
            self.sculk_get(pos)
                .is_some_and(|s| s.to_state().is_side_solid(face))
        }

        fn sculk_is_full_cube(&self, pos: BlockPos) -> bool {
            self.sculk_get(pos)
                .is_some_and(|s| s.to_state().is_full_cube())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::MockSculkLevel;

    #[test]
    fn can_spread_from_water_source_origin() {
        // 带有完整立方体邻居的水源起点是有效的扩散
        // 原点（回归问题：原型区块视图必须能检测水）。
        let mut level = MockSculkLevel::new();
        let origin = BlockPos::new(0, 60, 0);
        level.set_id(origin, Block::WATER.default_state.id);
        level.set_id(origin.up(), Block::STONE.default_state.id);
        assert!(can_spread_from(&level, origin));
    }

    #[test]
    fn can_spread_from_rejects_solid_origin() {
        let mut level = MockSculkLevel::new();
        let origin = BlockPos::new(0, 60, 0);
        level.set_id(origin, Block::STONE.default_state.id);
        assert!(!can_spread_from(&level, origin));
    }

    #[test]
    fn sculk_behaviour_implementations() {
        // 在这里只有幽匿块和幽匿脉络算作幽匿行为。
        assert!(is_sculk_behaviour(BlockId::SCULK));
        assert!(is_sculk_behaviour(BlockId::SCULK_VEIN));
        assert!(!is_sculk_behaviour(BlockId::SCULK_CATALYST));
        assert!(!is_sculk_behaviour(BlockId::SCULK_SENSOR));
        assert!(!is_sculk_behaviour(BlockId::CALIBRATED_SCULK_SENSOR));
        assert!(!is_sculk_behaviour(BlockId::SCULK_SHRIEKER));
    }

    #[test]
    fn can_spread_from_rejects_waterlogged_non_water_origin() {
        // 扩散需要空气或真正的水方块；含水的
        // 台阶（水流体、非水方块）且有一个完整方块邻居
        // 不得开始蔓延。
        struct WaterloggedSlabLevel {
            inner: MockSculkLevel,
            slab: BlockPos,
        }
        impl SculkLevel for WaterloggedSlabLevel {
            fn sculk_get(&self, pos: BlockPos) -> Option<BlockStateId> {
                self.inner.sculk_get(pos)
            }
            fn sculk_set(&mut self, pos: BlockPos, state: &'static BlockState) {
                self.inner.sculk_set(pos, state);
            }
            fn sculk_is_air(&self, pos: BlockPos) -> bool {
                self.inner.sculk_is_air(pos)
            }
            fn sculk_is_water_source(&self, pos: BlockPos) -> bool {
                // 建模 `GenerationCache` 视图：板块保存一份水
                // 源流体，尽管它并不是水方块。
                pos == self.slab || self.inner.sculk_is_water_source(pos)
            }
            fn sculk_is_water(&self, pos: BlockPos) -> bool {
                pos == self.slab || self.inner.sculk_is_water(pos)
            }
            fn sculk_is_face_sturdy(&self, pos: BlockPos, face: BlockDirection) -> bool {
                self.inner.sculk_is_face_sturdy(pos, face)
            }
            fn sculk_is_full_cube(&self, pos: BlockPos) -> bool {
                self.inner.sculk_is_full_cube(pos)
            }
        }

        let origin = BlockPos::new(0, 60, 0);
        let mut inner = MockSculkLevel::new();
        inner.set_id(origin, Block::OAK_SLAB.default_state.id);
        inner.set_id(origin.up(), Block::STONE.default_state.id);
        let level = WaterloggedSlabLevel {
            inner,
            slab: origin,
        };
        assert!(level.sculk_is_water_source(origin));
        assert!(!can_spread_from(&level, origin));
    }

    #[test]
    fn can_spread_from_rejects_sensor_origin() {
        // 传感器既非幽匿行为也非空气/水，因此传播
        // 无法以它为起点。
        let mut level = MockSculkLevel::new();
        let origin = BlockPos::new(0, 60, 0);
        level.set_id(origin, Block::SCULK_SENSOR.default_state.id);
        assert!(!can_spread_from(&level, origin));
    }
}
