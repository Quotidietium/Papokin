use crate::math::{vector2::Vector2, vector3::Axis};

use super::{position::BlockPos, vector3::Vector3};

/// 表示三维空间中的轴对齐包围盒。
#[derive(Clone, Copy, Debug)]
pub struct BoundingBox {
    /// 盒体的最小角点。
    pub min: Vector3<f64>,
    /// 盒体的最大角点。
    pub max: Vector3<f64>,
}

/// 表示用于碰撞检测的二维包围平面。
#[derive(Clone, Copy, Debug)]
struct BoundingPlane {
    /// 平面的最小角点。
    pub min: Vector2<f64>,
    /// 平面的最大角点。
    pub max: Vector2<f64>,
}

impl BoundingPlane {
    /// 检查此平面是否与另一平面相交。
    ///
    /// # Arguments
    /// * `other` – 用于检查的另一个边界平面。
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }

    /// 通过排除一个轴，将 3D 边界框投影到 2D 平面上。
    ///
    /// # Arguments
    /// * `bounding_box` – 要投影的 3D 包围盒。
    /// * `excluded` – 投影时要排除的轴。
    pub const fn from_box(bounding_box: &BoundingBox, excluded: Axis) -> Self {
        let [axis1, axis2] = Axis::excluding(excluded);

        Self {
            min: Vector2::new(
                bounding_box.get_side(false).get_axis(axis1),
                bounding_box.get_side(false).get_axis(axis2),
            ),

            max: Vector2::new(
                bounding_box.get_side(true).get_axis(axis1),
                bounding_box.get_side(true).get_axis(axis2),
            ),
        }
    }
}

impl BoundingBox {
    /// 使用实体尺寸在原点创建默认边界框。
    ///
    /// # Arguments
    /// * `size` – 实体的尺寸。
    #[must_use]
    pub fn new_default(size: &EntityDimensions) -> Self {
        Self::new_from_pos(0., 0., 0., size)
    }

    /// 根据位置和实体尺寸创建边界框。
    ///
    /// # Arguments
    /// * `x` – 位置的 X 坐标。
    /// * `y` – 位置的 Y 坐标。
    /// * `z` – 位置的 Z 坐标。
    /// * `size` – 实体的尺寸。
    #[must_use]
    pub fn new_from_pos(x: f64, y: f64, z: f64, size: &EntityDimensions) -> Self {
        let f = f64::from(size.width) / 2.;
        Self {
            min: Vector3::new(x - f, y, z - f),
            max: Vector3::new(x + f, y + f64::from(size.height), z + f),
        }
    }

    /// 沿每个轴按给定数量扩展此包围盒。
    ///
    /// # Arguments
    /// * `x` – 沿 X 轴扩展的量。
    /// * `y` – 沿 Y 轴扩展的量。
    /// * `z` – 沿 Z 轴扩展的量。
    #[must_use]
    pub fn expand(&self, x: f64, y: f64, z: f64) -> Self {
        Self {
            min: Vector3::new(self.min.x - x, self.min.y - y, self.min.z - z),
            max: Vector3::new(self.max.x + x, self.max.y + y, self.max.z + z),
        }
    }

    /// 将此包围盒朝特定方向扩展。
    ///
    /// 若提供的值为负，则沿该轴扩展最小边界。
    /// 若提供的值为正，则沿该轴扩展最大边界。
    ///
    /// # Arguments
    /// * `x` – 沿 X 轴方向扩展的量。
    /// * `y` – 沿 Y 轴方向扩展的量。
    /// * `z` – 沿 Z 轴方向扩展的量。
    #[must_use]
    pub fn expand_towards(&self, x: f64, y: f64, z: f64) -> Self {
        let mut min_x = self.min.x;
        let mut min_y = self.min.y;
        let mut min_z = self.min.z;

        let mut max_x = self.max.x;
        let mut max_y = self.max.y;
        let mut max_z = self.max.z;

        if x < 0.0 {
            min_x += x;
        } else if x > 0.0 {
            max_x += x;
        }

        if y < 0.0 {
            min_y += y;
        } else if y > 0.0 {
            max_y += y;
        }

        if z < 0.0 {
            min_z += z;
        } else if z > 0.0 {
            max_z += z;
        }

        Self {
            min: Vector3::new(min_x, min_y, min_z),
            max: Vector3::new(max_x, max_y, max_z),
        }
    }

    /// 沿所有轴均匀扩展此包围盒。
    ///
    /// # Arguments
    /// * `value` – 沿所有轴扩展的量。
    #[must_use]
    pub fn expand_all(&self, value: f64) -> Self {
        self.expand(value, value, value)
    }

    /// 将此盒子沿所有轴均匀收缩。
    ///
    /// # Arguments
    /// * `value` – 沿所有轴收缩的量。
    #[must_use]
    pub fn contract_all(&self, value: f64) -> Self {
        self.expand_all(-value)
    }

    ///返回一个平移到指定方块位置的新包围盒。
    ///
    /// # Arguments
    /// * `pos` – 要将盒体移动到的方块位置。
    #[must_use]
    pub fn at_pos(&self, pos: BlockPos) -> Self {
        let vec3 = Vector3 {
            x: f64::from(pos.0.x),
            y: f64::from(pos.0.y),
            z: f64::from(pos.0.z),
        };
        Self {
            min: self.min + vec3,
            max: self.max + vec3,
        }
    }

    ///返回一个按另一个包围盒偏移后的新包围盒。
    ///
    /// # Arguments
    /// * `other` – 要作为偏移添加的边界框。
    #[must_use]
    pub fn offset(&self, other: Self) -> Self {
        Self {
            min: self.min.add(&other.min),
            max: self.max.add(&other.max),
        }
    }

    /// 根据显式给出的最小和最大坐标创建边界框。
    ///
    /// # Arguments
    /// * `min` – 盒体的最小角点。
    /// * `max` – 盒体的最大角点。
    #[must_use]
    pub const fn new(min: Vector3<f64>, max: Vector3<f64>) -> Self {
        Self { min, max }
    }

    /// 根据最小和最大坐标数组创建边界框。
    ///
    /// # Arguments
    /// * `min` – 以数组 [x, y, z] 表示的最小角点。
    /// * `max` – 以数组 [x, y, z] 表示的最大角点。
    #[must_use]
    pub const fn new_array(min: [f64; 3], max: [f64; 3]) -> Self {
        Self {
            min: Vector3::new(min[0], min[1], min[2]),
            max: Vector3::new(max[0], max[1], max[2]),
        }
    }

    ///返回表示一个完整方块的包围盒，即从 (0,0,0) 到 (1,1,1)。
    #[must_use]
    pub const fn full_block() -> Self {
        Self {
            min: Vector3::new(0f64, 0f64, 0f64),
            max: Vector3::new(1f64, 1f64, 1f64),
        }
    }

    /// 从覆盖整个方块的方块位置创建边界框。
    ///
    /// # Arguments
    /// * `position` – 作为边界框基准的方块位置。
    #[must_use]
    pub fn from_block(position: &BlockPos) -> Self {
        let position = position.0;
        Self {
            min: Vector3::new(
                f64::from(position.x),
                f64::from(position.y),
                f64::from(position.z),
            ),
            max: Vector3::new(
                f64::from(position.x) + 1.0,
                f64::from(position.y) + 1.0,
                f64::from(position.z) + 1.0,
            ),
        }
    }

    /// 返回包围盒的最小侧或最大侧。
    ///
    /// # Arguments
    /// * `max` – 是否返回最大侧（true）还是最小侧（false）。
    #[must_use]
    pub const fn get_side(&self, max: bool) -> Vector3<f64> {
        if max { self.max } else { self.min }
    }

    /// 计算沿运动向量与另一包围盒发生碰撞的时间。
    ///
    /// # Arguments
    /// * `other` – 用于碰撞测试的边界框。
    /// * `movement` – 此盒体的移动向量。
    /// * `axis` – 计算碰撞所沿的轴。
    /// * `max_time` – 允许的最大碰撞时间。
    ///
    /// # Returns
    /// 若在 `max_time` 内发生碰撞则为 Some(f64)，否则为 None。
    #[must_use]
    pub fn calculate_collision_time(
        &self,
        other: &Self,
        movement: Vector3<f64>,
        axis: Axis,
        max_time: f64, // NOTE: 从 1.0 开始
    ) -> Option<f64> {
        let movement_on_axis = movement.get_axis(axis);

        if movement_on_axis == 0.0 {
            return None;
        }

        let move_positive = movement_on_axis.is_sign_positive();
        let self_plane_const = self.get_side(move_positive).get_axis(axis);
        let other_plane_const = other.get_side(!move_positive).get_axis(axis);
        let collision_time = (other_plane_const - self_plane_const) / movement_on_axis;

        if collision_time < 0.0 || collision_time >= max_time {
            return None;
        }

        let self_moved = self.shift(movement * collision_time);
        let self_plane_moved = BoundingPlane::from_box(&self_moved, axis);
        let other_plane = BoundingPlane::from_box(other, axis);

        if !self_plane_moved.intersects(&other_plane) {
            return None;
        }

        Some(collision_time)
    }

    ///返回包围盒的平均边长。
    #[must_use]
    pub fn get_average_side_length(&self) -> f64 {
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let depth = self.max.z - self.min.z;

        (width + height + depth) / 3.0
    }

    /// 返回此包围盒覆盖的最小方块位置。
    #[must_use]
    pub const fn min_block_pos(&self) -> BlockPos {
        BlockPos::floored_v(self.min)
    }

    /// 返回此包围盒覆盖的最大方块位置。
    #[must_use]
    pub const fn max_block_pos(&self) -> BlockPos {
        // 使用极小的 epsilon 并对最大坐标向下取整，使某个盒子
        // 最大值恰好落在方块边界上时不会包含相邻的
        // 方块。这与原版行为一致，原版中 max block 为闭区间
        // 仅当实体实际与该方块重叠时。
        let eps = 1e-9f64;
        BlockPos::floored_v(Vector3::new(
            self.max.x - eps,
            self.max.y - eps,
            self.max.z - eps,
        ))
    }

    ///返回一个按增量向量平移后的新包围盒。
    ///
    /// # Arguments
    /// * `delta` – 用于平移包围盒的向量。
    #[must_use]
    pub fn shift(&self, delta: Vector3<f64>) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    /// 按给定向量沿各轴拉伸此包围盒。
    ///
    /// # Arguments
    /// * `other` – 指定沿每个轴拉伸幅度的向量。
    #[must_use]
    pub const fn stretch(&self, other: Vector3<f64>) -> Self {
        let mut new = *self;

        if other.x < 0.0 {
            new.min.x += other.x;
        } else if other.x > 0.0 {
            new.max.x += other.x;
        }

        if other.y < 0.0 {
            new.min.y += other.y;
        } else if other.y > 0.0 {
            new.max.y += other.y;
        }

        if other.z < 0.0 {
            new.min.z += other.z;
        } else if other.z > 0.0 {
            new.max.z += other.z;
        }

        new
    }

    /// 根据方块位置创建零体积边界框。
    ///
    /// # Arguments
    /// * `position` – 作为边界框基准的方块位置。
    #[must_use]
    pub fn from_block_raw(position: &BlockPos) -> Self {
        let position = position.0;
        Self {
            min: Vector3::new(
                f64::from(position.x),
                f64::from(position.y),
                f64::from(position.z),
            ),
            max: Vector3::new(
                f64::from(position.x),
                f64::from(position.y),
                f64::from(position.z),
            ),
        }
    }

    /// 检查此包围盒是否与另一个包围盒相交。
    ///
    /// # Arguments
    /// * `other` – 用于检查的另一个边界框。
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    /// 计算从某一点到此包围盒最近点的距离平方。
    ///
    /// # Arguments
    /// * `pos` – 测量起点。
    #[must_use]
    pub fn squared_magnitude(&self, pos: Vector3<f64>) -> f64 {
        let d = f64::max(f64::max(self.min.x - pos.x, pos.x - self.max.x), 0.0);
        let e = f64::max(f64::max(self.min.y - pos.y, pos.y - self.max.y), 0.0);
        let f = f64::max(f64::max(self.min.z - pos.z, pos.z - self.max.z), 0.0);

        super::squared_magnitude(d, e, f)
    }
}

/// 表示实体的尺寸。
#[derive(Clone, Copy, Debug)]
pub struct EntityDimensions {
    /// 实体的宽度。
    pub width: f32,
    /// 实体的高度。
    pub height: f32,
    /// 相对于实体底部的眼睛高度。
    pub eye_height: f32,
}

impl EntityDimensions {
    /// 创建新的实体尺寸对象。
    ///
    /// # Arguments
    /// * `width` – 实体的宽度。
    /// * `height` – 实体的高度。
    /// * `eye_height` – 实体的眼睛高度。
    #[must_use]
    pub const fn new(width: f32, height: f32, eye_height: f32) -> Self {
        Self {
            width,
            height,
            eye_height,
        }
    }
}
