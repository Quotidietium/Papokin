use super::{
    get_section_cord,
    vector3::{self, Vector3},
};
use std::fmt;
use std::hash::Hash;

use crate::math::vector2::Vector2;
use num_traits::Euclid;
use papokin_codecs::codec::list::validate_fixed_size;
use papokin_codecs::{DataResult, FlatTryFrom, IntStream, comap_flat_map_codec_impl};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 产出一个长方体区域内所有 `BlockPos` 坐标的迭代器。
pub struct BlockPosIterator {
    /// 最小 X 坐标（含）。
    start_x: i32,
    /// 最小 Y 坐标（含）。
    start_y: i32,
    /// 最小 Z 坐标（含）。
    start_z: i32,
    /// 最大 X 坐标（含）。
    end_x: i32,
    /// 最大 Y 坐标（含）。
    end_y: i32,
    /// 当前的迭代索引。
    index: usize,
    /// 要迭代的位置总数。
    count: usize,
}

impl BlockPosIterator {
    /// 创建遍历指定闭长方体区域的新 `BlockPosIterator`。
    ///
    /// # Arguments
    /// - `start_x` – 最小 X 坐标（含）。
    /// - `start_y` – 最小 Y 坐标（含）。
    /// - `start_z` – 最小 Z 坐标（含）。
    /// - `end_x` – 最大 X 坐标（含）。
    /// - `end_y` – 最大 Y 坐标（含）。
    /// - `end_z` – 最大 Z 坐标（含）。
    ///
    /// # Returns
    /// 一个新的 `BlockPosIterator` 实例。
    #[must_use]
    pub const fn new(
        start_x: i32,
        start_y: i32,
        start_z: i32,
        end_x: i32,
        end_y: i32,
        end_z: i32,
    ) -> Self {
        let count_x = end_x - start_x + 1;
        let count_y = end_y - start_y + 1;
        let count_z = end_z - start_z + 1;
        let count = (count_x * count_y * count_z) as usize;
        Self {
            start_x,
            start_y,
            start_z,
            end_x,
            end_y,
            index: 0,
            count,
        }
    }
}

impl Iterator for BlockPosIterator {
    type Item = BlockPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }

        let size_x = (self.end_x - self.start_x + 1) as usize;
        let size_y = (self.end_y - self.start_y + 1) as usize;

        let x_offset = self.index % size_x;
        let y_offset = (self.index / size_x) % size_y;
        let z_offset = (self.index / size_x) / size_y;

        let x = self.start_x + x_offset as i32;
        let y = self.start_y + y_offset as i32;
        let z = self.start_z + z_offset as i32;

        self.index += 1;
        Some(BlockPos::new(x, y, z))
    }
}

/// 从中心点向外逐个产出 `BlockPos` 坐标的迭代器。
pub struct OutwardIterator {
    /// 中心点的 X 坐标。
    center_x: i32,
    /// 中心点的 Y 坐标。
    center_y: i32,
    /// 中心点的 Z 坐标。
    center_z: i32,
    /// 距中心的最大绝对 X 差值。
    range_x: i32,
    /// 距中心的最大绝对 Y 差值。
    range_y: i32,
    /// 距中心的最大绝对 Z 差值。
    range_z: i32,
    /// 要遍历的最大曼哈顿距离。
    max_manhattan_distance: i32,
    /// 当前正在考虑的位置。
    pos: BlockPos,
    /// 当前距中心的曼哈顿距离。
    manhattan_distance: i32,
    /// 该曼哈顿距离下当前的 X 限制。
    limit_x: i32,
    /// 该曼哈顿距离下当前的 Y 限制。
    limit_y: i32,
    /// 当前的 X 偏移量。
    dx: i32,
    /// 当前的 Y 偏移量。
    dy: i32,
    /// 是否为下一个位置交换 Z 的符号。
    swap_z: bool,
}

impl OutwardIterator {
    /// 创建从中心点向外产出位置的新 `OutwardIterator`。
    ///
    /// # Arguments
    /// - `center` – 开始迭代的中心 `BlockPos`。
    /// - `range_x` – X 坐标相对中心允许的最大绝对偏差。
    /// - `range_y` – Y 坐标相对中心允许的最大绝对偏差。
    /// - `range_z` – Z 坐标相对中心允许的最大绝对偏差。
    ///
    /// # Returns
    /// 一个新的 `OutwardIterator` 实例。
    #[must_use]
    pub const fn new(center: BlockPos, range_x: i32, range_y: i32, range_z: i32) -> Self {
        let max_manhattan_distance = range_x + range_y + range_z;
        Self {
            center_x: center.0.x,
            center_y: center.0.y,
            center_z: center.0.z,
            range_x,
            range_y,
            range_z,
            max_manhattan_distance,
            pos: BlockPos::ZERO,
            manhattan_distance: 0,
            limit_x: 0,
            limit_y: 0,
            dx: 0,
            dy: 0,
            swap_z: false,
        }
    }
}

impl Iterator for OutwardIterator {
    type Item = BlockPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.swap_z {
            self.swap_z = false;
            self.pos.0.z = self.center_z - (self.pos.0.z - self.center_z);
            return Some(self.pos);
        }

        loop {
            if self.dy > self.limit_y {
                self.dx += 1;
                if self.dx > self.limit_x {
                    self.manhattan_distance += 1;
                    if self.manhattan_distance > self.max_manhattan_distance {
                        return None; // endOfData()
                    }
                    self.limit_x = self.range_x.min(self.manhattan_distance);
                    self.dx = -self.limit_x;
                }
                self.limit_y = self.range_y.min(self.manhattan_distance - self.dx.abs());
                self.dy = -self.limit_y;
            }

            let i2 = self.dx;
            let j2 = self.dy;
            let k2 = self.manhattan_distance - i2.abs() - j2.abs();

            if k2 <= self.range_z {
                self.swap_z = k2 != 0;
                self.pos =
                    BlockPos::new(self.center_x + i2, self.center_y + j2, self.center_z + k2);
                self.dy += 1;
                return Some(self.pos);
            }
            self.dy += 1;
        }
    }
}

/// 表示 XZ 平面上二维方块网格中的一个位置。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ColumnPos(pub Vector2<i32>);

/// 表示三维方块网格中的一个位置。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockPos(pub Vector3<i32>);

impl BlockPos {
    /// 零位置（0, 0, 0）。
    pub const ZERO: Self = Self::new(0, 0, 0);

    /// 使用给定坐标创建新的 `BlockPos`。
    ///
    /// # Arguments
    /// - `x` – X 坐标。
    /// - `y` – Y 坐标。
    /// - `z` – Z 坐标。
    ///
    /// # Returns
    /// 一个新的 `BlockPos` 实例。
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(Vector3::new(x, y, z))
    }

    #[must_use]
    pub const fn containing(x: f64, y: f64, z: f64) -> Self {
        Self::floored(x, y, z)
    }

    #[must_use]
    pub const fn containing_vec(pos: Vector3<f64>) -> Self {
        Self::floored_v(pos)
    }

    #[must_use]
    pub fn min(a: Self, b: Self) -> Self {
        Self::new(a.0.x.min(b.0.x), a.0.y.min(b.0.y), a.0.z.min(b.0.z))
    }

    #[must_use]
    pub fn max(a: Self, b: Self) -> Self {
        Self::new(a.0.x.max(b.0.x), a.0.y.max(b.0.y), a.0.z.max(b.0.z))
    }

    /// 遍历由两个角点定义的长方体区域内的所有 `BlockPos`。
    #[must_use]
    pub fn iterate(start: Self, end: Self) -> BlockPosIterator {
        BlockPosIterator::new(
            start.0.x.min(end.0.x),
            start.0.y.min(end.0.y),
            start.0.z.min(end.0.z),
            start.0.x.max(end.0.x),
            start.0.y.max(end.0.y),
            start.0.z.max(end.0.z),
        )
    }

    #[must_use]
    pub fn between_closed(start: Self, end: Self) -> BlockPosIterator {
        Self::iterate(start, end)
    }

    #[must_use]
    pub fn between_closed_coords(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> BlockPosIterator {
        BlockPosIterator::new(
            min_x.min(max_x),
            min_y.min(max_y),
            min_z.min(max_z),
            min_x.max(max_x),
            min_y.max(max_y),
            min_z.max(max_z),
        )
    }

    /// 从指定的 `center` 点向外遍历 `BlockPos` 对象。
    ///
    /// # Arguments
    /// - `center` – 开始迭代的中心 `BlockPos`。
    /// - `range_x` – X 坐标相对中心允许的最大绝对偏差。
    /// - `range_y` – Y 坐标相对中心允许的最大绝对偏差。
    /// - `range_z` – Z 坐标相对中心允许的最大绝对偏差。
    ///
    /// # Returns
    /// 一个 `OutwardIterator`，按所述的向外顺序产出 `BlockPos` 实例。
    #[must_use]
    pub const fn iterate_outwards(
        center: Self,
        range_x: i32,
        range_y: i32,
        range_z: i32,
    ) -> OutwardIterator {
        OutwardIterator::new(center, range_x, range_y, range_z)
    }

    /// 从指定的 `center` 点向外遍历 `BlockPos` 对象。
    /// 这是 [`Self::iterate_outwards`] 的借用版本。
    ///
    /// # Arguments
    /// - `center` – 开始迭代的中心 `&BlockPos`。
    /// - `range_x` – X 坐标相对中心允许的最大绝对偏差。
    /// - `range_y` – Y 坐标相对中心允许的最大绝对偏差。
    /// - `range_z` – Z 坐标相对中心允许的最大绝对偏差。
    ///
    /// # Returns
    /// 一个 `OutwardIterator`，按所述的向外顺序产出 `BlockPos` 实例。
    #[must_use]
    pub const fn iterate_outwards_ref(
        center: &Self,
        range_x: i32,
        range_y: i32,
        range_z: i32,
    ) -> OutwardIterator {
        Self::iterate_outwards(*center, range_x, range_y, range_z)
    }

    /// 创建遍历指定闭区间的方块位置迭代器。
    ///
    /// # Arguments
    /// - `start_x` – 最小 X 坐标（含）。
    /// - `start_y` – 最小 Y 坐标（含）。
    /// - `start_z` – 最小 Z 坐标（含）。
    /// - `end_x` – 最大 X 坐标（含）。
    /// - `end_y` – 最大 Y 坐标（含）。
    /// - `end_z` – 最大 Z 坐标（含）。
    ///
    /// # Returns
    /// 一个 `BlockPosIterator`，产出定义范围内的每个 `BlockPos`。
    #[must_use]
    pub const fn iterate_block_pos(
        start_x: i32,
        start_y: i32,
        start_z: i32,
        end_x: i32,
        end_y: i32,
        end_z: i32,
    ) -> BlockPosIterator {
        BlockPosIterator::new(start_x, start_y, start_z, end_x, end_y, end_z)
    }

    ///同时返回区块坐标和该区块内的相对位置。
    ///
    /// # Returns
    /// 一个包含以下内容的元组：
    /// - 以 `Vector2<i32>` 表示的区块位置。
    /// - 以 `Vector3<i32>` 表示的区块内相对位置。
    #[must_use]
    pub const fn chunk_and_chunk_relative_position(&self) -> (Vector2<i32>, Vector3<i32>) {
        (self.chunk_position(), self.chunk_relative_position())
    }

    ///返回包含此方块的区块坐标。
    ///
    /// # Returns
    /// 一个 `Vector2<i32>`，表示区块坐标（仅 X 和 Z）。
    #[must_use]
    pub const fn chunk_position(&self) -> Vector2<i32> {
        let z_chunk = self.0.z.div_euclid(16);
        let x_chunk = self.0.x.div_euclid(16);
        Vector2 {
            x: x_chunk,
            y: z_chunk,
        }
    }

    /// 返回此方块相对于其所在区块的位置。
    ///
    /// # Returns
    /// 一个 `Vector3<i32>`，X 和 Z 坐标在 [0, 15] 范围内，
    /// 以及实际的 Y 坐标（因为区块在 Y 方向上无限延伸）。
    #[must_use]
    pub const fn chunk_relative_position(&self) -> Vector3<i32> {
        let z_chunk = self.0.z.rem_euclid(16);
        let x_chunk = self.0.x.rem_euclid(16);
        Vector3 {
            x: x_chunk,
            y: self.0.y,
            z: z_chunk,
        }
    }

    /// 返回此方块相对于其所在区段的位置。
    ///
    /// # Returns
    /// 一个 `Vector3<i32>`，所有坐标都在 [0, 15] 范围内。
    #[must_use]
    pub fn section_relative_position(&self) -> Vector3<i32> {
        let (_z_chunk, z_rem) = self.0.z.div_rem_euclid(&16);
        let (_x_chunk, x_rem) = self.0.x.div_rem_euclid(&16);
        let (_y_chunk, y_rem) = self.0.y.div_rem_euclid(&16);

        // NOTE: 由于除以 16，余数不可能超过 u8
        Vector3 {
            x: x_rem,
            z: z_rem,
            y: y_rem,
        }
    }

    /// 从打包的 64 位整数表示创建 `BlockPos`。
    ///
    /// 打包格式如下：
    /// - 位 38-63: X 坐标（26 位）
    /// - 位 12-37: Z 坐标（26 位）
    /// - 位 0-11: Y 坐标（12 位）
    ///
    /// # Arguments
    /// - `encoded_position` – 打包后的 64 位整数。
    ///
    /// # Returns
    /// 解码得到的 `BlockPos`。
    #[must_use]
    pub const fn from_i64(encoded_position: i64) -> Self {
        Self(Vector3 {
            x: (encoded_position >> 38) as i32,
            y: (encoded_position << 52 >> 52) as i32,
            z: (encoded_position << 26 >> 38) as i32,
        })
    }

    /// 对给定浮点坐标向下取整以创建 `BlockPos`。
    ///
    /// # Arguments
    /// - `x` – 以浮点数表示的 X 坐标。
    /// - `y` – 以浮点数表示的 Y 坐标。
    /// - `z` – 以浮点数表示的 Z 坐标。
    ///
    /// # Returns
    /// 一个 `BlockPos`，每个坐标都向下取整为最近的整数。
    #[must_use]
    pub const fn floored(x: f64, y: f64, z: f64) -> Self {
        Self(Vector3::new(
            x.floor() as i32,
            y.floor() as i32,
            z.floor() as i32,
        ))
    }

    /// 对给定向量向下取整以创建 `BlockPos`。
    ///
    /// # Arguments
    /// - `pos` – 要向下取整的 `Vector3<f64>`。
    ///
    /// # Returns
    /// 一个 `BlockPos`，每个分量都向下取整为最近的整数。
    #[must_use]
    pub const fn floored_v(pos: Vector3<f64>) -> Self {
        Self(Vector3::new(
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        ))
    }

    /// 对给定向量向上取整以创建 `BlockPos`。
    ///
    /// # Arguments
    /// - `pos` – 要向上取整的 `Vector3<f64>`。
    ///
    /// # Returns
    /// 一个 `BlockPos`，每个分量都向上取整为最近的整数。
    #[must_use]
    pub const fn ceiled_v(pos: Vector3<f64>) -> Self {
        Self(Vector3::new(
            pos.x.ceil() as i32,
            pos.y.ceil() as i32,
            pos.z.ceil() as i32,
        ))
    }

    /// 将此方块位置转换为 `Vector3<f64>`，并在 X 和 Z 轴上各加 0.5。
    ///
    /// 这会将位置在方块的 X 和 Z 轴上居中，同时保持
    /// 精确的 Y 坐标。
    ///
    /// # Returns
    /// 一个 `Vector3<f64>`，表示居中的方块位置。
    #[must_use]
    pub fn to_f64(&self) -> Vector3<f64> {
        Vector3::new(
            f64::from(self.0.x) + 0.5,
            f64::from(self.0.y),
            f64::from(self.0.z) + 0.5,
        )
    }

    /// 将此方块位置转换为 `Vector3<f64>`，并在所有轴上各加 0.5。
    ///
    /// # Returns
    /// 一个 `Vector3<f64>`，表示完全居中的方块位置。
    #[must_use]
    pub fn to_centered_f64(&self) -> Vector3<f64> {
        Vector3::new(
            f64::from(self.0.x) + 0.5,
            f64::from(self.0.y) + 0.5,
            f64::from(self.0.z) + 0.5,
        )
    }

    /// 向此方块位置添加向量偏移。
    ///
    /// # Arguments
    /// - `offset` – 要加上的 `Vector3<i32>` 偏移量。
    ///
    /// # Returns
    /// 位于偏移位置的新 `BlockPos`。
    #[must_use]
    pub fn offset(&self, offset: Vector3<i32>) -> Self {
        Self(self.0 + offset)
    }

    /// 向此方块位置添加原始坐标偏移。
    ///
    /// # Arguments
    /// - `x` – X 偏移量。
    /// - `y` – Y 偏移量。
    /// - `z` – Z 偏移量。
    ///
    /// # Returns
    /// 位于偏移位置的新 `BlockPos`。
    #[must_use]
    pub const fn add(&self, x: i32, y: i32, z: i32) -> Self {
        Self::new(self.0.x + x, self.0.y + y, self.0.z + z)
    }

    /// 添加一个乘以系数的方向偏移。
    ///
    /// # Arguments
    /// - `offset` – 基础偏移向量。
    /// - `direction` – 应用于偏移量的乘数。
    ///
    /// # Returns
    /// 位于偏移位置的新 `BlockPos`。
    #[must_use]
    pub const fn offset_dir(&self, offset: Vector3<i32>, direction: i32) -> Self {
        Self(Vector3::new(
            self.0.x + offset.x * direction,
            self.0.y + offset.y * direction,
            self.0.z + offset.z * direction,
        ))
    }

    ///返回上方一格（Y+1）的方块位置。
    ///
    /// # Returns
    /// 位于 (x, y+1, z) 的新 `BlockPos`。
    #[must_use]
    pub fn up(&self) -> Self {
        self.offset(Vector3::new(0, 1, 0))
    }

    ///返回上方数格处的方块位置。
    ///
    /// # Arguments
    /// - `height` – 向上移动的方块数。
    ///
    /// # Returns
    /// 位于 (x, y+height, z) 的新 `BlockPos`。
    #[must_use]
    pub fn up_height(&self, height: i32) -> Self {
        self.offset(Vector3::new(0, height, 0))
    }

    ///返回下方一格（Y-1）的方块位置。
    ///
    /// # Returns
    /// 位于 (x, y-1, z) 的新 `BlockPos`。
    #[must_use]
    pub fn down(&self) -> Self {
        self.offset(Vector3::new(0, -1, 0))
    }

    ///返回下方数格处的方块位置。
    ///
    /// # Arguments
    /// - `height` – 向下移动的方块数。
    ///
    /// # Returns
    /// 位于 (x, y-height, z) 的新 `BlockPos`。
    #[must_use]
    pub fn down_height(&self, height: i32) -> Self {
        self.offset(Vector3::new(0, -height, 0))
    }

    ///返回西方一格（X-1）的方块位置。
    ///
    /// # Returns
    /// 位于 (x-1, y, z) 的新 `BlockPos`。
    #[must_use]
    pub fn west(&self) -> Self {
        self.offset(Vector3::new(-1, 0, 0))
    }

    ///返回北方一格（Z-1）的方块位置。
    ///
    /// # Returns
    /// 位于 (x, y, z-1) 的新 `BlockPos`。
    #[must_use]
    pub fn north(&self) -> Self {
        self.offset(Vector3::new(0, 0, -1))
    }

    ///返回东方一格（X+1）的方块位置。
    ///
    /// # Returns
    /// 位于 (x+1, y, z) 的新 `BlockPos`。
    #[must_use]
    pub fn east(&self) -> Self {
        self.offset(Vector3::new(1, 0, 0))
    }

    ///返回南方一格（Z+1）的方块位置。
    ///
    /// # Returns
    /// 位于 (x, y, z+1) 的新 `BlockPos`。
    #[must_use]
    pub fn south(&self) -> Self {
        self.offset(Vector3::new(0, 0, 1))
    }

    /// 计算此方块位置与另一位置之间的曼哈顿距离。
    ///
    /// # Arguments
    /// - `other` – 另一个方块位置。
    ///
    /// # Returns
    /// 曼哈顿距离（各轴上绝对差之和）。
    #[must_use]
    pub const fn manhattan_distance(&self, other: Self) -> i32 {
        let x = (other.0.x - self.0.x).abs();
        let y = (other.0.y - self.0.y).abs();
        let z = (other.0.z - self.0.z).abs();
        x + y + z
    }

    /// 计算此方块位置与另一位置之间的欧几里得距离平方。
    ///
    /// # Arguments
    /// - `other` – 另一个方块位置。
    ///
    /// # Returns
    /// 欧几里得距离的平方。
    #[must_use]
    pub fn squared_distance(&self, other: &Self) -> i32 {
        self.0.squared_distance_to_vec(&other.0)
    }

    /// 计算从此方块中心 `(x + 0.5, y + 0.5, z + 0.5)` 到 `(x, y, z)` 的距离平方。
    #[must_use]
    pub fn dist_to_center_sqr(&self, x: f64, y: f64, z: f64) -> f64 {
        let dx = f64::from(self.0.x) + 0.5 - x;
        let dy = f64::from(self.0.y) + 0.5 - y;
        let dz = f64::from(self.0.z) + 0.5 - z;
        dx * dx + dy * dy + dz * dz
    }

    /// 将此方块位置打包为 64 位整数。
    ///
    /// 打包格式如下：
    /// - 位 38-63: X 坐标（26 位）
    /// - 位 12-37: Z 坐标（26 位）
    /// - 位 0-11: Y 坐标（12 位）
    ///
    /// # Returns
    /// 一个表示此方块位置的 64 位打包整数。
    #[must_use]
    pub fn as_long(&self) -> i64 {
        ((i64::from(self.0.x) & 0x03FF_FFFF) << 38)
            | ((i64::from(self.0.z) & 0x03FF_FFFF) << 12)
            | (i64::from(self.0.y) & 0xFFF)
    }

    /// 根据 Minecraft 版本将此方块位置打包为 64 位整数。
    ///
    /// - 对于 1.14+：打包为 X（26 位）、Z（26 位）、Y（12 位）
    /// - 对于 1.8 - 1.13.2：打包为 X（26 位）、Y（12 位）、Z（26 位）
    #[must_use]
    pub fn as_long_for_version(&self, version: &crate::version::JavaMinecraftVersion) -> i64 {
        if *version >= crate::version::JavaMinecraftVersion::V_1_14 {
            self.as_long()
        } else {
            ((i64::from(self.0.x) & 0x03FF_FFFF) << 38)
                | ((i64::from(self.0.y) & 0xFFF) << 26)
                | (i64::from(self.0.z) & 0x03FF_FFFF)
        }
    }

    /// 根据 Minecraft 版本将 64 位整数解包为 `BlockPos`。
    ///
    /// - 对于 1.14+：从 X（26 位）、Z（26 位）、Y（12 位）解包
    /// - 对于 1.8 - 1.13.2：从 X（26 位）、Y（12 位）、Z（26 位）解包
    #[must_use]
    pub fn from_long_for_version(val: i64, version: &crate::version::JavaMinecraftVersion) -> Self {
        if *version >= crate::version::JavaMinecraftVersion::V_1_14 {
            Self::new(
                (val >> 38) as i32,
                (val << 52 >> 52) as i32,
                (val << 26 >> 38) as i32,
            )
        } else {
            Self::new(
                (val >> 38) as i32,
                (val << 26 >> 52) as i32,
                (val << 38 >> 38) as i32,
            )
        }
    }
}

impl Serialize for BlockPos {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.as_long())
    }
}

impl<'de> Deserialize<'de> for BlockPos {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = BlockPos;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("An i64 int")
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(BlockPos(Vector3 {
                    x: (v >> 38) as i32,
                    y: (v << 52 >> 52) as i32,
                    z: (v << 26 >> 38) as i32,
                }))
            }
        }
        deserializer.deserialize_i64(Visitor)
    }
}

impl From<&BlockPos> for IntStream {
    fn from(value: &BlockPos) -> Self {
        let Vector3 { x, y, z } = value.0;
        Self(vec![x, y, z])
    }
}

impl FlatTryFrom<IntStream> for BlockPos {
    fn flat_try_from(value: IntStream) -> DataResult<Self> {
        validate_fixed_size(value.0, 3).flat_map(|v| {
            v.try_into().map_or_else(
                |_| DataResult::new_error("Expected 3 elements"),
                |arr| {
                    let [x, y, z]: [i32; 3] = arr;
                    DataResult::new_success(Self(Vector3::new(x, y, z)))
                },
            )
        })
    }
}

comap_flat_map_codec_impl!(IntStream => BlockPos, BlockPos::flat_try_from, IntStream::from);

impl fmt::Display for BlockPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}, {}", self.0.x, self.0.y, self.0.z)
    }
}

///返回包含给定方块位置的区块段位置。
///
/// # Arguments
/// - `block_pos` – 方块位置。
///
/// # Returns
/// 一个 `Vector3<i32>`，表示区块段的坐标。
#[must_use]
pub const fn chunk_section_from_pos(block_pos: &BlockPos) -> Vector3<i32> {
    let block_pos = block_pos.0;
    Vector3::new(
        get_section_cord(block_pos.x),
        get_section_cord(block_pos.y),
        get_section_cord(block_pos.z),
    )
}

/// 获取区块段内的局部坐标（0-15）。
///
/// # Arguments
/// - `cord` – 全局坐标。
///
/// # Returns
/// 局部坐标（位 0-3），范围为 [0, 15]。
#[must_use]
pub const fn get_local_cord(cord: i32) -> i32 {
    cord & 15
}

/// 将区块区段内的局部坐标打包为 16 位整数。
///
/// 打包格式与 `vector3::packed_local` 一致：
/// - 位 8-15: X 坐标（4 位）
/// - 位 4-7: Z 坐标（4 位）
/// - 位 0-3: Y 坐标（4 位）
///
/// # Arguments
/// - `block_pos` – 方块位置。
///
/// # Returns
/// 一个包含局部坐标的 16 位打包整数。
#[must_use]
pub const fn pack_local_chunk_section(block_pos: &BlockPos) -> i16 {
    let x = get_local_cord(block_pos.0.x);
    let y = get_local_cord(block_pos.0.y);
    let z = get_local_cord(block_pos.0.z);
    vector3::packed_local(&Vector3::new(x, y, z))
}

#[cfg(test)]
mod test {
    use crate::math::position::BlockPos;
    use papokin_codecs::{assert_decode, assert_encode_success};
    use papokin_nbt::nbt_ops::NbtOps;
    use papokin_nbt::tag::NbtTag;

    #[test]
    fn codec() {
        assert_encode_success!(
            BlockPos::new(1, 2, 3),
            NbtOps,
            NbtTag::IntArray(vec![1, 2, 3])
        );
        assert_encode_success!(
            BlockPos::new(-1000, 200, 4521),
            NbtOps,
            NbtTag::IntArray(vec![-1000, 200, 4521])
        );

        assert_decode!(
            BlockPos,
            NbtTag::IntArray(vec![1, 2, 3]),
            NbtOps,
            is_success
        );

        assert_decode!(BlockPos, NbtTag::List(vec![]), NbtOps, is_error);
        assert_decode!(
            BlockPos,
            NbtTag::List(vec![NbtTag::Int(1), NbtTag::Float(2.0), NbtTag::Int(3)]),
            NbtOps,
            is_success
        );
        assert_decode!(
            BlockPos,
            NbtTag::List(vec![
                NbtTag::Int(1),
                NbtTag::Float(2.0),
                NbtTag::String("69".into())
            ]),
            NbtOps,
            is_error
        );
        assert_decode!(
            BlockPos,
            NbtTag::List(vec![
                NbtTag::Int(1),
                NbtTag::Float(2.0),
                NbtTag::Int(3),
                NbtTag::Byte(3)
            ]),
            NbtOps,
            is_error
        );
    }

    #[test]
    fn iterate_outwards_ref_matches_value_version() {
        let center = BlockPos::new(1, 1, 1);

        let by_value: Vec<BlockPos> = BlockPos::iterate_outwards(center, 1, 1, 1).collect();
        let by_reference: Vec<BlockPos> =
            BlockPos::iterate_outwards_ref(&center, 1, 1, 1).collect();

        assert_eq!(by_reference, by_value);
    }
}
