use crate::{
    BlockDirection,
    math::{
        position::BlockPos,
        vector3::{Axis, Vector3},
    },
};
use papokin_codecs::codec::list::validate_fixed_size;
use papokin_codecs::{DataResult, FlatTryFrom, IntStream, comap_flat_map_codec_impl};

/// 表示整数坐标下轴对齐的三维方块包围盒。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockBox {
    /// 盒体的最小角点（含）。
    pub min: Vector3<i32>,
    /// 盒体的最大角点（含）。
    pub max: Vector3<i32>,
}

impl BlockBox {
    /// 根据最小/最大坐标创建新盒子。
    ///
    /// # Arguments
    /// * `min_x` – 最小 X 坐标（含）。
    /// * `min_y` – 最小 Y 坐标（含）。
    /// * `min_z` – 最小 Z 坐标（含）。
    /// * `max_x` – 最大 X 坐标（含）。
    /// * `max_y` – 最大 Y 坐标（含）。
    /// * `max_z` – 最大 Z 坐标（含）。
    #[must_use]
    pub const fn new(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> Self {
        Self {
            min: Vector3 {
                x: min_x,
                y: min_y,
                z: min_z,
            },
            max: Vector3 {
                x: max_x,
                y: max_y,
                z: max_z,
            },
        }
    }

    /// 沿某一轴创建具有指定尺寸的盒子。
    ///
    /// # Arguments
    /// * `x`、`y`、`z` – 起始坐标。
    /// * `axis` – 箱体延伸所沿的轴。
    /// * `width`、`height`、`depth` – 盒体的尺寸。
    #[must_use]
    pub fn create_box(
        x: i32,
        y: i32,
        z: i32,
        axis: Axis,
        width: i32,
        height: i32,
        depth: i32,
    ) -> Self {
        if axis == Axis::Z {
            Self::new(x, y, z, x + width - 1, y + height - 1, z + depth - 1)
        } else {
            Self::new(x, y, z, x + depth - 1, y + height - 1, z + width - 1)
        }
    }

    /// 根据偏移量和尺寸创建一个旋转后的箱体。
    ///
    /// # Arguments
    /// * `x`、`y`、`z` – 基准坐标。
    /// * `offset_x`、`offset_y`、`offset_z` – 相对基准的偏移量。
    /// * `size_x`、`size_y`、`size_z` – 尺寸。
    /// * `facing` – 朝向。
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub const fn rotated(
        x: i32,
        y: i32,
        z: i32,
        offset_x: i32,
        offset_y: i32,
        offset_z: i32,
        size_x: i32,
        size_y: i32,
        size_z: i32,
        facing: &BlockDirection,
    ) -> Self {
        match facing {
            BlockDirection::North => Self::new(
                x + offset_x,
                y + offset_y,
                z - size_z + 1 + offset_z,
                x + size_x - 1 + offset_x,
                y + size_y - 1 + offset_y,
                z + offset_z,
            ),
            BlockDirection::West => Self::new(
                x - size_z + 1 + offset_z,
                y + offset_y,
                z + offset_x,
                x + offset_z,
                y + size_y - 1 + offset_y,
                z + size_x - 1 + offset_x,
            ),
            BlockDirection::East => Self::new(
                x + offset_z,
                y + offset_y,
                z + offset_x,
                x + size_z - 1 + offset_z,
                y + size_y - 1 + offset_y,
                z + size_x - 1 + offset_x,
            ),
            // 默认 / 南
            _ => Self::new(
                x + offset_x,
                y + offset_y,
                z + offset_z,
                x + size_x - 1 + offset_x,
                y + size_y - 1 + offset_y,
                z + size_z - 1 + offset_z,
            ),
        }
    }

    /// 创建覆盖单个方块位置的盒子。
    ///
    /// # Arguments
    /// * `pos` – 方块位置。
    #[must_use]
    pub const fn from_pos(pos: BlockPos) -> Self {
        Self {
            min: pos.0,
            max: pos.0,
        }
    }

    /// 在所有方向上扩展此包围盒。
    ///
    /// # Arguments
    /// * `x`、`y`、`z` – 在每个轴上扩展的量。
    #[must_use]
    pub const fn expand(&self, x: i32, y: i32, z: i32) -> Self {
        Self {
            min: Vector3::new(self.min.x - x, self.min.y - y, self.min.z - z),
            max: Vector3::new(self.max.x + x, self.max.y + y, self.max.z + z),
        }
    }

    /// 将方块盒按给定的偏移量移动。
    ///
    /// # Arguments
    /// * `dx`、`dy`、`dz` – 偏移量。
    pub const fn move_pos(&mut self, dx: i32, dy: i32, dz: i32) {
        self.min.x += dx;
        self.min.y += dy;
        self.min.z += dz;
        self.max.x += dx;
        self.max.y += dy;
        self.max.z += dz;
    }

    ///若该包围盒包含给定的方块位置，则返回 `true`。
    ///
    /// # Arguments
    /// * `pos` – 方块坐标。
    #[must_use]
    pub const fn contains_pos(&self, pos: &Vector3<i32>) -> bool {
        self.contains(pos.x, pos.y, pos.z)
    }

    ///若该包围盒包含给定的坐标，则返回 `true`。
    ///
    /// # Arguments
    /// * `x`、`y`、`z` – 要测试的坐标。
    #[must_use]
    pub const fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        x >= self.min.x
            && x <= self.max.x
            && y >= self.min.y
            && y <= self.max.y
            && z >= self.min.z
            && z <= self.max.z
    }

    ///若该包围盒与另一个包围盒在三维空间中相交，则返回 `true`。
    ///
    /// # Arguments
    /// * `other` – 要测试的另一个盒体。
    #[must_use]
    pub const fn intersects(&self, other: &Self) -> bool {
        self.max.x >= other.min.x
            && self.min.x <= other.max.x
            && self.max.z >= other.min.z
            && self.min.z <= other.max.z
            && self.max.y >= other.min.y
            && self.min.y <= other.max.y
    }

    ///若该包围盒与另一个包围盒在 XZ 平面上相交，则返回 `true`。
    ///
    /// # Arguments
    /// * `other` – 要测试的另一个盒体。
    #[must_use]
    pub const fn intersects_xz(&self, other: &Self) -> bool {
        self.max.x >= other.min.x
            && self.min.x <= other.max.x
            && self.max.z >= other.min.z
            && self.min.z <= other.max.z
    }

    ///若该包围盒与给定的原始 XZ 坐标相交，则返回 `true`。
    ///
    /// # Arguments
    /// * `min_x`、`min_z`、`max_x`、`max_z` – 原始 XZ 边界。
    #[must_use]
    pub const fn intersects_raw_xz(&self, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> bool {
        self.max.x >= min_x && self.min.x <= max_x && self.max.z >= min_z && self.min.z <= max_z
    }

    /// 返回沿 Y 轴的方块数量。
    #[must_use]
    pub const fn get_block_count_y(&self) -> i32 {
        self.max.y - self.min.y + 1
    }

    /// 扩展此包围盒以包含另一个包围盒。
    ///
    /// # Arguments
    /// * `other` – 要包含在内的盒体。
    pub fn encompass(&mut self, other: &Self) {
        self.min.x = self.min.x.min(other.min.x);
        self.min.y = self.min.y.min(other.min.y);
        self.min.z = self.min.z.min(other.min.z);
        self.max.x = self.max.x.max(other.max.x);
        self.max.y = self.max.y.max(other.max.y);
        self.max.z = self.max.z.max(other.max.z);
    }

    ///返回覆盖迭代器中所有包围盒的包围盒。
    ///
    /// # Arguments
    /// * `boxes` – 箱体的迭代器。
    /// # Returns
    /// * 覆盖所有箱体的 `Some(Box)`，若为空则为 `None`。
    pub fn encompass_all<I>(boxes: I) -> Option<Self>
    where
        I: IntoIterator<Item = Self>,
    {
        let mut iter = boxes.into_iter();
        let mut result = iter.next()?; // NOTE: 若为空则返回 None

        for b in iter {
            result.encompass(&b);
        }
        Some(result)
    }
}

impl From<&BlockBox> for IntStream {
    fn from(value: &BlockBox) -> Self {
        let Vector3 {
            x: min_x,
            y: min_y,
            z: min_z,
        } = value.min;
        let Vector3 {
            x: max_x,
            y: max_y,
            z: max_z,
        } = value.max;
        Self(vec![min_x, min_y, min_z, max_x, max_y, max_z])
    }
}

impl FlatTryFrom<IntStream> for BlockBox {
    fn flat_try_from(value: IntStream) -> DataResult<Self> {
        validate_fixed_size(value.0, 6).flat_map(|v| {
            v.try_into().map_or_else(
                |_| DataResult::new_error("Expected 6 elements"),
                |arr| {
                    let [min_x, min_y, min_z, max_x, max_y, max_z]: [i32; 6] = arr;
                    DataResult::new_success(Self {
                        min: Vector3::new(min_x, min_y, min_z),
                        max: Vector3::new(max_x, max_y, max_z),
                    })
                },
            )
        })
    }
}

comap_flat_map_codec_impl!(IntStream => BlockBox, BlockBox::flat_try_from, IntStream::from);
