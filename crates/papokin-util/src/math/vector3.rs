use bytes::BufMut;
use std::ops::{Add, AddAssign, Div, Mul, Sub};

use super::position::BlockPos;
use super::vector2::Vector2;
use crate::math::vector_codec_impl;
use num_traits::{Float, Num};
use papokin_codecs::codec::list::validate_fixed_size;
use papokin_codecs::{DataResult, Decode, DynamicOps, Encode, FlatTryFrom};

/// 分量类型为 `T` 的三维向量。
#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq, Default)]
pub struct Vector3<T> {
    /// 向量的 X 分量。
    pub x: T,
    /// 向量的 Y 分量。
    pub y: T,
    /// 向量的 Z 分量。
    pub z: T,
}

/// 表示三维空间中的一条主轴。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Axis {
    /// X 轴。
    X,
    /// Y 轴。
    Y,
    /// Z 轴。
    Z,
}

impl Axis {
    ///返回全部三个轴 `[Y, X, Z]`。
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Y, Self::X, Self::Z]
    }

    ///返回水平轴 `[X, Z]`。
    #[must_use]
    pub const fn horizontal() -> [Self; 2] {
        [Self::X, Self::Z]
    }

    /// 返回除给定 `axis` 之外的两个轴。
    ///
    /// # Arguments
    /// - `axis` – 要排除的轴。
    ///
    /// # Returns
    /// 一个包含除给定轴之外另两个轴的数组。
    #[must_use]
    pub const fn excluding(axis: Self) -> [Self; 2] {
        match axis {
            Self::X => [Self::Y, Self::Z],

            Self::Y => [Self::X, Self::Z],

            Self::Z => [Self::X, Self::Y],
        }
    }
}

impl<T: Copy> Vector3<T> {
    /// 获取指定轴的分量值。
    ///
    /// # Arguments
    /// - `a` – 要获取其值的轴。
    ///
    /// # Returns
    /// 指定轴上的分量值。
    pub const fn get_axis(&self, a: Axis) -> T {
        match a {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// 设置指定轴上的分量值。
    ///
    /// # Arguments
    /// - `a` – 要设置其值的轴。
    /// - `value` – 该轴的新值。
    pub const fn set_axis(&mut self, a: Axis, value: T) {
        match a {
            Axis::X => self.x = value,
            Axis::Y => self.y = value,
            Axis::Z => self.z = value,
        }
    }
}

impl<T> Vector3<T> {
    /// 使用给定分量创建新的 `Vector3`。
    ///
    /// # Arguments
    /// - `x` – X 分量。
    /// - `y` – Y 分量。
    /// - `z` – Z 分量。
    ///
    /// # Returns
    /// 一个具有指定分量的 `Vector3`。
    #[must_use]
    pub const fn new(x: T, y: T, z: T) -> Self {
        Self { x, y, z }
    }
}

impl Vector3<f64> {
    #[must_use]
    pub fn from_yaw_pitch(yaw: f32, pitch: f32) -> Self {
        let yaw_rad = f64::from(yaw).to_radians();
        let pitch_rad = f64::from(pitch).to_radians();

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        Self::new(-cos_pitch * sin_yaw, -sin_pitch, cos_pitch * cos_yaw)
    }
}

impl<T: Math + PartialOrd + Copy> Vector3<T> {
    /// 计算向量长度的平方（模的平方）。
    ///
    /// # Returns
    /// 向量的长度的平方。
    #[must_use]
    pub fn length_squared(&self) -> T {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// 计算水平长度的平方（仅 XZ 分量）。
    ///
    /// # Returns
    /// 向量的水平长度的平方。
    #[must_use]
    pub fn horizontal_length_squared(&self) -> T {
        self.x * self.x + self.z * self.z
    }

    /// 将另一个向量加到此向量上并返回结果。
    ///
    /// # Arguments
    /// - `other` – 要加上的向量。
    ///
    /// # Returns
    /// 一个表示按分量相加结果的新向量。
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    /// 将原始分量值加到此向量上并返回结果。
    ///
    /// # Arguments
    /// - `x` – 要加上的 X 值。
    /// - `y` – 要加上的 Y 值。
    /// - `z` – 要加上的 Z 值。
    ///
    /// # Returns
    /// 一个各分量加上原始值的新向量。
    #[must_use]
    pub fn add_raw(&self, x: T, y: T, z: T) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            z: self.z + z,
        }
    }

    /// 从此向量减去另一向量并返回结果。
    ///
    /// # Arguments
    /// - `other` – 要减去的向量。
    ///
    /// # Returns
    /// 一个表示按分量相减结果的新向量。
    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    /// 从此向量减去原始分量值并返回结果。
    ///
    /// # Arguments
    /// - `x` – 要减去的 X 值。
    /// - `y` – 要减去的 Y 值。
    /// - `z` – 要减去的 Z 值。
    ///
    /// # Returns
    /// 一个各分量减去原始值的新向量。
    #[must_use]
    pub fn sub_raw(&self, x: T, y: T, z: T) -> Self {
        Self {
            x: self.x - x,
            y: self.y - y,
            z: self.z - z,
        }
    }

    /// 将此向量与原始分量值逐项相乘并返回结果。
    ///
    /// # Arguments
    /// - `x` – X 乘数。
    /// - `y` – Y 乘数。
    /// - `z` – Z 乘数。
    ///
    /// # Returns
    /// 一个各分量乘以对应原始值的新向量。
    #[must_use]
    pub fn multiply(self, x: T, y: T, z: T) -> Self {
        Self {
            x: self.x * x,
            y: self.y * y,
            z: self.z * z,
        }
    }

    /// 在此向量与另一向量之间执行线性插值。
    ///
    /// # Arguments
    /// - `other` – 插值朝向的目标向量。
    /// - `t` – 插值因子（0.0 = 本向量，1.0 = 另一个向量）。
    ///
    /// # Returns
    /// 给定系数 `t` 下的插值向量。
    #[must_use]
    pub fn lerp(&self, other: &Self, t: T) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }

    ///返回一个各分量为 -1、0 或 1 的向量，指示每个分量的符号。
    ///
    /// # Returns
    /// 一个 `Vector3<i32>`，其每个分量为：
    /// - 若原分量为正则返回 `1`。
    /// - 若原分量为负则返回 `-1`。
    /// - 若原分量为零则返回 `0`。
    #[must_use]
    pub fn sign(&self) -> Vector3<i32>
    where
        T: Num + PartialOrd + Copy,
    {
        Vector3 {
            x: if self.x > T::zero() {
                1
            } else if self.x < T::zero() {
                -1
            } else {
                0
            },
            y: if self.y > T::zero() {
                1
            } else if self.y < T::zero() {
                -1
            } else {
                0
            },
            z: if self.z > T::zero() {
                1
            } else if self.z < T::zero() {
                -1
            } else {
                0
            },
        }
    }

    /// 计算此向量与另一向量之间的距离平方。
    ///
    /// # Arguments
    /// - `other` – 另一个向量。
    ///
    /// # Returns
    /// 两个向量之间的欧几里得距离的平方。
    #[must_use]
    pub fn squared_distance_to_vec(&self, other: &Self) -> T {
        self.squared_distance_to(other.x, other.y, other.z)
    }

    /// 计算此向量与给定坐标之间的距离平方。
    ///
    /// # Arguments
    /// - `x` – X 坐标。
    /// - `y` – Y 坐标。
    /// - `z` – Z 坐标。
    ///
    /// # Returns
    /// 到指定坐标的欧几里得距离的平方。
    #[must_use]
    pub fn squared_distance_to(&self, x: T, y: T, z: T) -> T {
        let delta_x = self.x - x;
        let delta_y = self.y - y;
        let delta_z = self.z - z;
        delta_x * delta_x + delta_y * delta_y + delta_z * delta_z
    }

    /// 计算此向量与另一向量之间的水平距离平方（仅 XZ）。
    ///
    /// # Arguments
    /// - `other` – 另一个向量。
    ///
    /// # Returns
    /// 两个向量之间的水平距离的平方。
    pub fn squared_distance_to_vec_xz(&self, other: Self) -> T {
        self.squared_distance_to_xz(other.x, other.z)
    }

    /// 计算此向量与给定坐标之间的水平距离平方（仅 XZ）。
    ///
    /// # Arguments
    /// - `x` – X 坐标。
    /// - `z` – Z 坐标。
    ///
    /// # Returns
    /// 到指定坐标的水平距离的平方。
    pub fn squared_distance_to_xz(&self, x: T, z: T) -> T {
        let delta_x = self.x - x;
        let delta_z = self.z - z;
        delta_x * delta_x + delta_z * delta_z
    }

    /// 检查此向量是否位于以给定位置为中心的长方体区域内。
    ///
    /// # Arguments
    /// - `block_pos` – 长方体的中心位置。
    /// - `x` – X 方向上的半宽。
    /// - `y` – Y 方向上的半高。
    /// - `z` – Z 方向上的半深。
    ///
    /// # Returns
    /// 如果向量在边界内则为 `true`，否则为 `false`。
    #[must_use]
    pub fn is_within_bounds(&self, block_pos: Self, x: T, y: T, z: T) -> bool {
        let min_x = block_pos.x - x;
        let max_x = block_pos.x + x;
        let min_y = block_pos.y - y;
        let max_y = block_pos.y + y;
        let min_z = block_pos.z - z;
        let max_z = block_pos.z + z;

        self.x >= min_x
            && self.x <= max_x
            && self.y >= min_y
            && self.y <= max_y
            && self.z >= min_z
            && self.z <= max_z
    }

    /// 计算此向量与给定坐标的点积。
    ///
    /// # Arguments
    /// - `x` – 用于点乘的 X 坐标。
    /// - `y` – 用于点乘的 Y 坐标。
    /// - `z` – 用于点乘的 Z 坐标。
    ///
    /// # Returns
    /// 点积 `self.x * x + self.y * y + self.z * z`。
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> T {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// 计算此向量与给定坐标的叉积。
    ///
    /// # Arguments
    /// - `x` – 用于叉乘的 X 坐标。
    /// - `y` – 用于叉乘的 Y 坐标。
    /// - `z` – 用于叉乘的 Z 坐标。
    ///
    /// # Returns
    /// 两个向量的叉积。
    #[inline]
    #[must_use]
    pub fn cross(&self, other: &Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
}

impl<T: Math + Copy + Float> Vector3<T> {
    /// 计算向量的长度（模）。
    ///
    /// # Returns
    /// 向量的长度。
    pub fn length(&self) -> T {
        self.length_squared().sqrt()
    }

    /// 计算向量的水平长度（XZ 分量的大小）。
    ///
    /// # Returns
    /// 向量的水平长度。
    pub fn horizontal_length(&self) -> T {
        self.horizontal_length_squared().sqrt()
    }

    ///返回此向量的归一化版本，或在无法归一化时返回零向量
    /// 无法进行规范化。
    ///
    /// 结果向量方向不变，但长度为 1。
    ///
    /// # Returns
    /// 一个指向相同方向的单位向量。
    #[must_use]
    pub fn normalize(&self) -> Self {
        let length_recip = self.length().recip();

        if length_recip.is_finite() && length_recip > T::zero() {
            Self {
                x: self.x * length_recip,
                y: self.y * length_recip,
                z: self.z * length_recip,
            }
        } else {
            Self {
                x: T::zero(),
                y: T::zero(),
                z: T::zero(),
            }
        }
    }

    /// 根据俯仰角和偏航角创建方向向量。
    ///
    /// # Arguments
    /// - `pitch` – 俯仰角，单位为度（上/下）。
    /// - `yaw` – 偏航角，单位为度（左/右）。
    ///
    /// # Returns
    /// 表示该方向的单位向量。
    pub fn rotation_vector(pitch: T, yaw: T) -> Self {
        let h = pitch.to_radians();
        let i = (-yaw).to_radians();

        let l = h.cos();
        Self {
            x: i.sin() * l,
            y: -h.sin(),
            z: i.cos() * l,
        }
    }
}

impl<T: Math + Copy> Mul<T> for Vector3<T> {
    type Output = Self;

    fn mul(self, scalar: T) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }
}

impl<T: Math + Copy> Div<T> for Vector3<T> {
    type Output = Self;

    fn div(self, scalar: T) -> Self {
        Self {
            x: self.x / scalar,
            y: self.y / scalar,
            z: self.z / scalar,
        }
    }
}

impl<T: Math + Copy> Add for Vector3<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl<T: Math + Copy> Sub for Vector3<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl<T: Math + Copy> AddAssign for Vector3<T> {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }
}

/*
impl<T: Math + Copy> Neg for Vector3<T> {
    type Output = Self;

    fn neg(self) -> Self {
        Vector3 {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}
*/

impl<T> From<(T, T, T)> for Vector3<T> {
    fn from((x, y, z): (T, T, T)) -> Self {
        Self { x, y, z }
    }
}

impl<T> From<Vector3<T>> for (T, T, T) {
    fn from(vector: Vector3<T>) -> Self {
        (vector.x, vector.y, vector.z)
    }
}

impl Vector3<f64> {
    /// 将此向量转换为 `Vector3<f32>`（有损）。
    ///
    /// # Returns
    /// 一个各分量被强制转换为 `f32` 的 `Vector3<f32>`。
    #[must_use]
    pub const fn to_f32_lossy(&self) -> Vector3<f32> {
        Vector3 {
            x: self.x as f32,
            y: self.y as f32,
            z: self.z as f32,
        }
    }
}

impl<T: Math + Copy + Into<f64>> Vector3<T> {
    /// 将此向量转换为 `Vector3<f64>`。
    ///
    /// # Returns
    /// 一个各分量被换算为 `f64` 的 `Vector3<f64>`。
    pub fn to_f64(&self) -> Vector3<f64> {
        Vector3 {
            x: self.x.into(),
            y: self.y.into(),
            z: self.z.into(),
        }
    }
}

impl<T: Math + Copy + Into<f32>> Vector3<T> {
    /// 将此向量转换为 `Vector3<f32>`。
    ///
    /// # Returns
    /// 一个各分量被换算为 `f32` 的 `Vector3<f32>`。
    pub fn to_f32(&self) -> Vector3<f32> {
        Vector3 {
            x: self.x.into(),
            y: self.y.into(),
            z: self.z.into(),
        }
    }
}

impl<T: Math + Copy + Into<f64>> Vector3<T> {
    /// 将每个分量四舍五入到最近的整数，并转换为 `Vector3<i32>`。
    ///
    /// # Returns
    /// 一个各分量四舍五入到最近整数的 `Vector3<i32>`。
    pub fn to_i32(&self) -> Vector3<i32> {
        let x: f64 = self.x.into();
        let y: f64 = self.y.into();
        let z: f64 = self.z.into();
        Vector3 {
            x: x.round() as i32,
            y: y.round() as i32,
            z: z.round() as i32,
        }
    }

    /// 将 X 和 Z 分量四舍五入到最近的整数，并转换为 2D 向量。
    ///
    /// # Returns
    /// 一个 X 和 Z 分量四舍五入到最近整数的 `Vector2<i32>`。
    pub fn to_vec2_i32(&self) -> Vector2<i32> {
        let x: f64 = self.x.into();
        let z: f64 = self.z.into();
        Vector2 {
            x: x.round() as i32,
            y: z.round() as i32,
        }
    }
}

impl<T: Math + Copy + Into<f64>> Vector3<T> {
    /// 将每个分量向下取整为最近整数并转换为 `Vector3<i32>`。
    ///
    /// # Returns
    /// 一个各分量向下取整到最近整数的 `Vector3<i32>`。
    pub fn floor_to_i32(&self) -> Vector3<i32> {
        let x: f64 = self.x.into();
        let y: f64 = self.y.into();
        let z: f64 = self.z.into();
        Vector3 {
            x: x.floor() as i32,
            y: y.floor() as i32,
            z: z.floor() as i32,
        }
    }

    /// 将 X 和 Z 分量向下取整为最近整数并转换为 2D 向量。
    ///
    /// # Returns
    /// 一个 X 和 Z 分量向下取整到最近整数的 `Vector2<i32>`。
    pub fn floor_to_vec2_i32(&self) -> Vector2<i32> {
        let x: f64 = self.x.into();
        let z: f64 = self.z.into();
        Vector2 {
            x: x.floor() as i32,
            y: z.floor() as i32,
        }
    }
}

impl<T: Math + Copy + Into<f64>> Vector3<T> {
    /// 将此向量转换为 `BlockPos`，每个分量四舍五入为最接近的整数。
    ///
    /// # Returns
    /// 表示取整后位置的新 `BlockPos`。
    pub fn to_block_pos(&self) -> BlockPos {
        BlockPos(self.to_i32())
    }
}

/// 表示向量分量所需基本数学运算的 trait。
pub trait Math:
    Mul<Output = Self>
    //+ Neg<Output = Self>
    + Add<Output = Self>
    + AddAssign<>
    + Div<Output = Self>
    + Sub<Output = Self>
    + Sized
{
}

impl Math for i16 {}
impl Math for f64 {}
impl Math for f32 {}
impl Math for i32 {}
impl Math for i64 {}
impl Math for u8 {}

impl<'de> serde::Deserialize<'de> for Vector3<i32> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vector3Visitor;

        impl<'de> serde::de::Visitor<'de> for Vector3Visitor {
            type Value = Vector3<i32>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid Vector<i32>")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                if let Some(x) = seq.next_element::<i32>()?
                    && let Some(y) = seq.next_element::<i32>()?
                    && let Some(z) = seq.next_element::<i32>()?
                {
                    return Ok(Vector3::new(x, y, z));
                }
                Err(serde::de::Error::custom("Failed to read Vector<i32>"))
            }
        }

        deserializer.deserialize_seq(Vector3Visitor)
    }
}

impl<'de> serde::Deserialize<'de> for Vector3<f32> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vector3Visitor;

        impl<'de> serde::de::Visitor<'de> for Vector3Visitor {
            type Value = Vector3<f32>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid Vector<32>")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                if let Some(x) = seq.next_element::<f32>()?
                    && let Some(y) = seq.next_element::<f32>()?
                    && let Some(z) = seq.next_element::<f32>()?
                {
                    return Ok(Vector3::new(x, y, z));
                }
                Err(serde::de::Error::custom("Failed to read Vector<f32>"))
            }
        }

        deserializer.deserialize_seq(Vector3Visitor)
    }
}

impl<'de> serde::Deserialize<'de> for Vector3<f64> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vector3Visitor;

        impl<'de> serde::de::Visitor<'de> for Vector3Visitor {
            type Value = Vector3<f64>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid Vector<f64>")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                if let Some(x) = seq.next_element::<f64>()?
                    && let Some(y) = seq.next_element::<f64>()?
                    && let Some(z) = seq.next_element::<f64>()?
                {
                    return Ok(Vector3::new(x, y, z));
                }
                Err(serde::de::Error::custom("Failed to read Vector<f64>"))
            }
        }

        deserializer.deserialize_seq(Vector3Visitor)
    }
}

impl serde::Serialize for Vector3<f32> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf = Vec::new();
        buf.put_f32(self.x);
        buf.put_f32(self.y);
        buf.put_f32(self.z);
        serializer.serialize_bytes(&buf)
    }
}

impl serde::Serialize for Vector3<f64> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf = Vec::new();
        buf.put_f64(self.x);
        buf.put_f64(self.y);
        buf.put_f64(self.z);
        serializer.serialize_bytes(&buf)
    }
}

impl serde::Serialize for Vector3<i16> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf = Vec::new();
        buf.put_i16(self.x);
        buf.put_i16(self.y);
        buf.put_i16(self.z);
        serializer.serialize_bytes(&buf)
    }
}

impl serde::Serialize for Vector3<i32> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf = Vec::new();
        buf.put_i32(self.x);
        buf.put_i32(self.y);
        buf.put_i32(self.z);
        serializer.serialize_bytes(&buf)
    }
}

vector_codec_impl!(Vector3<T>, 3, x, y, z);

/// 将区块位置向量打包为单个 64 位整数。
///
/// 打包格式如下：
/// - 位 42-63: X 坐标（22 位）
/// - 位 20-41: Z 坐标（22 位）
/// - 位 0-19: Y 坐标（20 位）
///
/// # Arguments
/// - `vec` – 要打包的区块位置向量。
///
/// # Returns
/// 一个包含编码后位置的 64 位打包整数。
#[inline]
#[must_use]
pub const fn packed_chunk_pos(vec: &Vector3<i32>) -> i64 {
    let mut result = 0i64;
    // NOTE: 需要先转为 i64 以保留符号。
    result |= (vec.x as i64 & 0x003F_FFFF) << 42;
    result |= (vec.z as i64 & 0x003F_FFFF) << 20;
    result |= vec.y as i64 & 0xFFFFF;
    result
}

/// 将 64 位整数解包为区块段位置向量。
#[inline]
#[must_use]
pub const fn unpacked_chunk_pos(packed: i64) -> Vector3<i32> {
    let x = (packed >> 42) as i32;
    let y = ((packed << 44) >> 44) as i32;
    let z = ((packed << 22) >> 42) as i32;
    Vector3::new(x, y, z)
}

/// 将区块内的局部位置打包为单个 16 位整数。
///
/// 打包格式如下：
/// - 位 8-15: X 坐标（4 位）
/// - 位 4-7: Z 坐标（4 位）
/// - 位 0-3: Y 坐标（4 位）
///
/// # Arguments
/// - `vec` – 要打包的局部位置向量。
///
/// # Returns
/// 一个包含编码后位置的 16 位打包整数。
#[inline]
#[must_use]
pub const fn packed_local(vec: &Vector3<i32>) -> i16 {
    let x = vec.x as i16;
    let y = vec.y as i16;
    let z = vec.z as i16;
    (x << 8) | (z << 4) | y
}
