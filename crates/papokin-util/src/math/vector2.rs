use std::ops::{Add, Div, Mul, Neg, Sub};

use super::vector3::Vector3;
use crate::math::vector_codec_impl;
use bytes::BufMut;
use num_traits::Float;
use papokin_codecs::codec::list::validate_fixed_size;
use papokin_codecs::{DataResult, Decode, DynamicOps, Encode, FlatTryFrom};

/// 具有泛型数值分量的二维向量。
#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq, Default)]
pub struct Vector2<T> {
    /// 向量的 X 分量。
    pub x: T,
    /// 向量的 Y 分量。
    pub y: T,
}

impl<T: Math + Copy> Vector2<T> {
    /// 使用给定的分量创建一个新向量。
    ///
    /// # Arguments
    /// * `x` – X 分量。
    /// * `y` – Y 分量。
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    /// 返回向量的长度平方。
    pub fn length_squared(&self) -> T {
        self.x * self.x + self.y * self.y
    }

    /// 返回此向量与另一向量之和。
    ///
    /// # Arguments
    /// * `other` – 要加上的向量。
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    /// 将原始分量值加到此向量上。
    ///
    /// # Arguments
    /// * `x` – 加到 X 上的值。
    /// * `y` – 加到 Y 上的值。
    #[must_use]
    pub fn add_raw(&self, x: T, y: T) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
        }
    }

    ///返回此向量与另一个向量之差。
    ///
    /// # Arguments
    /// * `other` – 要减去的向量。
    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    /// 将向量按分量逐项相乘。
    ///
    /// # Arguments
    /// * `x` – X 的乘数。
    /// * `y` – Y 的乘数。
    #[must_use]
    pub fn multiply(self, x: T, y: T) -> Self {
        Self {
            x: self.x * x,
            y: self.y * y,
        }
    }
}

impl<T: Math + Copy + Float> Vector2<T> {
    /// 返回向量的长度（模）。
    pub fn length(&self) -> T {
        self.length_squared().sqrt()
    }

    ///返回该向量的归一化版本，或在无法归一化时返回零向量
    /// 无法进行规范化。
    #[must_use]
    pub fn normalize(&self) -> Self {
        let length_recip = self.length().recip();

        if length_recip.is_finite() && length_recip > T::zero() {
            Self {
                x: self.x * length_recip,
                y: self.y * length_recip,
            }
        } else {
            Self {
                x: T::zero(),
                y: T::zero(),
            }
        }
    }
}

impl<T: Math + Copy> Mul<T> for Vector2<T> {
    type Output = Self;

    fn mul(self, scalar: T) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

impl<T: Math + Copy> Add for Vector2<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: Math + Copy> Neg for Vector2<T> {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl<T> From<(T, T)> for Vector2<T> {
    fn from((x, z): (T, T)) -> Self {
        Self { x, y: z }
    }
}

impl<T> From<Vector3<T>> for Vector2<T> {
    fn from(value: Vector3<T>) -> Self {
        Self {
            x: value.x,
            y: value.z,
        }
    }
}

/// 表示支持标准算术运算的数值类型的 trait。
pub trait Math:
    Mul<Output = Self>
    + Neg<Output = Self>
    + Add<Output = Self>
    + Div<Output = Self>
    + Sub<Output = Self>
    + Sized
{
}

impl Math for f64 {}
impl Math for f32 {}
impl Math for i32 {}
impl Math for i64 {}
impl Math for i8 {}

/// 将方块位置向量转换为区块位置向量。
///
/// # Arguments
/// * `vec` – 要转换的方块位置向量。
///
/// # Returns
/// 一个 `Vector2<i32>`，表示对应的区块位置。
#[must_use]
pub const fn to_chunk_pos(vec: &Vector2<i32>) -> Vector2<i32> {
    Vector2::new(vec.x >> 4, vec.y >> 4)
}

impl serde::Serialize for Vector2<f32> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        buf.put_f32(self.x);
        buf.put_f32(self.y);
        serializer.serialize_bytes(&buf)
    }
}

vector_codec_impl!(Vector2<T>, 2, x, y);
