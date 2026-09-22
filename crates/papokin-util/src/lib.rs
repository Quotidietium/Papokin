//! Pumpkin 服务器的工具函数与共享类型。

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::panic))]

use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

pub use serde_json;

pub use difficulty::Difficulty;
pub use gamemode::GameMode;
pub use permission::PermissionLvl;

use crate::{math::vector3::Axis, random::RandomImpl};

pub mod biome;
pub mod difficulty;
pub mod gamemode;
pub mod loot_table;
pub mod math;
pub mod noise;
pub mod permission;
pub mod random;
pub mod registry;
pub mod resource_location;
pub mod serde_enum_as_integer;
pub mod text;
pub mod translation;
pub mod version;
pub mod world_seed;
pub mod y_offset;

pub mod identifier;
pub mod resource;
pub mod uuid;

/// 表示用于地形生成和碰撞检测的各种高度图类型。
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HeightMap {
    /// 最顶部的方块，包括植物、雪层和地表特征（用于世界生成期间）。
    WorldSurfaceWg,
    /// 计为表面的最顶部固体或液体方块。
    WorldSurface,
    /// 海洋中最低的固体方块，包括水下地形特征（在世界生成期间使用）。
    OceanFloorWg,
    /// 海洋中最低的固体方块，忽略海带等非固体特征。
    OceanFloor,
    /// 阻挡实体移动的最顶部方块（忽略树叶）。
    MotionBlocking,
    /// 阻挡实体移动的最顶部方块，忽略树叶方块。
    MotionBlockingNoLeaves,
}

/// 构造相对于项目根目录的全局文件系统路径。
#[macro_export]
macro_rules! global_path {
    ($path:expr) => {{
        use std::path::Path;
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap_or_else(|| Path::new("."))
            .join(file!())
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join($path)
    }};
}

/// 从磁盘读取 JSON 文件。不要将它用于静态文件！
#[macro_export]
macro_rules! read_data_from_file {
    ($path:expr) => {{
        use $crate::global_path;
        $crate::serde_json::from_str(
            &std::fs::read_to_string(global_path!($path)).expect("找不到数据文件"),
        )
        .expect("数据解码失败")
    }};
}

/// 断言两个浮点数在给定误差范围内近似相等。
#[macro_export]
macro_rules! assert_eq_delta {
    ($x:expr, $y:expr, $d:expr) => {
        if 2f64 * ($x - $y).abs() > $d * ($x.abs() + $y.abs()) {
            panic!("{} vs {}（{} vs {}）", $x, $y, ($x - $y).abs(), $d);
        }
    };
}

/// 表示此数字所需的最小位数
#[inline]
#[must_use]
pub fn encompassing_bits(count: usize) -> u8 {
    if count == 1 {
        1
    } else {
        count.ilog2() as u8 + u8::from(!count.is_power_of_two())
    }
}

/// 表示对玩家档案施加的、可能需要审核或限制的操作。
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileAction {
    /// 玩家的名称被服务器或管理员强制更改。
    ForcedNameChange,
    /// 玩家尝试使用服务器禁止或不允许的皮肤。
    UsingBannedSkin,
}

/// 表示三维世界中方块朝向的六种可能方向。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlockDirection {
    /// 沿 Y 轴向下；常用于附着在天花板上的方块。
    Down = 0,
    /// 沿 Y 轴向上；常用于放置在地面上的方块。
    Up,
    /// 朝向 Z 轴负方向；通常指向结构的前方。
    North,
    /// 朝向 Z 轴正方向；通常指向结构的后方。
    South,
    /// 朝向 X 轴负方向；常用于朝左的方向。
    West,
    /// 朝向 X 轴正方向；常用于朝右的方向。
    East,
}

impl BlockDirection {
    /// 返回与此方向关联的主轴（`X`、`Y` 或 `Z`）。
    #[must_use]
    pub const fn get_axis(&self) -> Axis {
        match self {
            Self::Up | Self::Down => Axis::Y,
            Self::North | Self::South => Axis::Z,
            Self::East | Self::West => Axis::X,
        }
    }

    pub fn get_random_horizontal_direction(random: &mut impl RandomImpl) -> Self {
        match random.next_bounded_i32(4) {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            _ => Self::West,
        }
    }

    #[must_use]
    pub const fn opposite(&self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    #[must_use]
    pub const fn to_vector(&self) -> crate::math::vector3::Vector3<i32> {
        match self {
            Self::Down => crate::math::vector3::Vector3::new(0, -1, 0),
            Self::Up => crate::math::vector3::Vector3::new(0, 1, 0),
            Self::North => crate::math::vector3::Vector3::new(0, 0, -1),
            Self::South => crate::math::vector3::Vector3::new(0, 0, 1),
            Self::West => crate::math::vector3::Vector3::new(-1, 0, 0),
            Self::East => crate::math::vector3::Vector3::new(1, 0, 0),
        }
    }
}

/// 一个可变切片，拆分为三部分：分割索引处的元素、起始部分和结束部分。
///
/// 这允许在仍可访问周围切片的同时修改所选元素。
pub struct MutableSplitSlice<'a, T> {
    /// 分割索引之前的元素。
    start: &'a mut [T],
    /// 分割索引之后的元素。
    end: &'a mut [T],
}

impl<'a, T> MutableSplitSlice<'a, T> {
    /// 将第 `index` 个元素提取为可变引用，同时返回一个表示其余元素的 `MutableSplitSlice`。
    ///
    /// # Panics
    /// * 若 `index` 超出基础切片的边界。
    #[allow(clippy::expect_used, clippy::panic)]
    pub const fn extract_ith(base: &'a mut [T], index: usize) -> (&'a mut T, Self) {
        let (start, end_inclusive) = base.split_at_mut(index);
        let Some((value, end)) = end_inclusive.split_first_mut() else {
            panic!("索引不在基础切片内");
        };

        (value, Self { start, end })
    }

    /// 返回切分切片中的元素总数（起始 + 被移除元素 + 结尾）。
    #[must_use]
    pub const fn len(&self) -> usize {
        self.start.len() + self.end.len() + 1
    }

    ///返回 `false`，因为切分后的切片总是至少包含被移除的元素。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        false
    }
}

impl<T> Index<usize> for MutableSplitSlice<'_, T> {
    type Output = T;

    #[expect(clippy::comparison_chain)]
    #[allow(clippy::panic)]
    fn index(&self, index: usize) -> &Self::Output {
        if index < self.start.len() {
            &self.start[index]
        } else if index == self.start.len() {
            panic!("尝试索引到已被移除的元素");
        } else {
            &self.end[index - self.start.len() - 1]
        }
    }
}

/// 用于反序列化双 Perlin 噪声采样器参数的编解码器。
#[derive(Deserialize, Clone)]
pub struct DoublePerlinNoiseParametersCodec {
    /// 第一个八度索引（可以为负，表示更低频率）。
    #[serde(rename = "firstOctave")]
    pub first_octave: i32,
    /// 每个八度的振幅值，决定各频率层的权重。
    pub amplitudes: Vec<f64>,
}

impl<T> IndexMut<usize> for MutableSplitSlice<'_, T> {
    #[expect(clippy::comparison_chain)]
    #[allow(clippy::panic)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index < self.start.len() {
            &mut self.start[index]
        } else if index == self.start.len() {
            panic!("尝试索引到已被移除的元素");
        } else {
            &mut self.end[index - self.start.len() - 1]
        }
    }
}

/// 表示玩家的主手。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    /// 通常是玩家的副手。
    Left,
    /// 通常是玩家的主手。
    Right,
}

impl Hand {
    #[must_use]
    pub const fn all() -> [Self; 2] {
        [Self::Right, Self::Left]
    }

    /// 转换游戏数据包的 `hand` 字段，其中 `0` 表示主手。
    ///
    /// 这与 [`TryFrom<i32>`] 相反，后者读取主手
    /// 自客户端设置读取，其中 `0` 表示左手。
    ///
    /// # Errors
    ///若值不是 0 或 1，则返回 `InvalidHand`。
    pub const fn from_packet_id(value: i32) -> Result<Self, InvalidHand> {
        match value {
            0 => Ok(Self::Right),
            1 => Ok(Self::Left),
            _ => Err(InvalidHand),
        }
    }
}

/// 手部转换无效时的错误类型。
pub struct InvalidHand;

impl TryFrom<i32> for Hand {
    type Error = InvalidHand;

    /// 将整数转换为 `Hand`。
    ///
    /// # Parameters
    /// - `0`：`Left`
    /// - `1`：`Right`
    ///
    /// # Errors
    ///若值不是 0 或 1，则返回 `InvalidHand`。
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Left),
            1 => Ok(Self::Right),
            _ => Err(InvalidHand),
        }
    }
}
