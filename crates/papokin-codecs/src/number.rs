use core::fmt;
use std::fmt::{Display, Formatter};

/// 表示 Java 中的通用数字。
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Number {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

// 对齐 Java 数值窄化语义：浮点取整截断为有意行为
#[allow(clippy::cast_possible_truncation)]
impl From<Number> for i64 {
    fn from(num: Number) -> Self {
        match num {
            Number::Byte(b) => Self::from(b),
            Number::Short(s) => Self::from(s),
            Number::Int(i) => Self::from(i),
            Number::Long(l) => l,
            Number::Float(f) => f as Self,
            Number::Double(d) => d as Self,
        }
    }
}

// 对齐 Java 数值窄化语义：截断为有意行为
#[allow(clippy::cast_possible_truncation)]
impl From<Number> for i32 {
    fn from(num: Number) -> Self {
        match num {
            Number::Byte(b) => Self::from(b),
            Number::Short(s) => Self::from(s),
            Number::Int(i) => i,
            Number::Long(l) => l as Self,
            Number::Float(f) => f as Self,
            Number::Double(d) => d as Self,
        }
    }
}

// 对齐 Java 数值窄化语义：截断为有意行为
#[allow(clippy::cast_possible_truncation)]
impl From<Number> for i16 {
    fn from(num: Number) -> Self {
        // 与 Java 类似，我们先将数字转换为 `i16`，再转换为 `i8`。
        i32::from(num) as Self
    }
}

// 对齐 Java 数值窄化语义：截断为有意行为
#[allow(clippy::cast_possible_truncation)]
impl From<Number> for i8 {
    fn from(num: Number) -> Self {
        // 与 Java 类似，我们先将数字转换为 `i32`，再转换为 `i8`。
        i32::from(num) as Self
    }
}

// 对齐 Java 数值窄化语义：截断与符号丢失为有意行为
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl From<Number> for u8 {
    fn from(num: Number) -> Self {
        i32::from(num) as Self
    }
}

// 对齐 Java 转换语义：i32/i64→f32 精度损失与 f64→f32 截断为有意行为
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
impl From<Number> for f32 {
    fn from(num: Number) -> Self {
        match num {
            Number::Byte(b) => Self::from(b),
            Number::Short(s) => Self::from(s),
            Number::Int(i) => i as Self,
            Number::Long(l) => l as Self,
            Number::Float(f) => f,
            Number::Double(d) => d as Self,
        }
    }
}

// 对齐 Java 转换语义：i64→f64 精度损失为有意行为
#[allow(clippy::cast_precision_loss)]
impl From<Number> for f64 {
    fn from(num: Number) -> Self {
        match num {
            Number::Byte(b) => Self::from(b),
            Number::Short(s) => Self::from(s),
            Number::Int(i) => Self::from(i),
            Number::Long(l) => l as Self,
            Number::Float(f) => Self::from(f),
            Number::Double(d) => d,
        }
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Byte(v) => write!(f, "{v}"),
            Self::Short(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Long(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Double(v) => write!(f, "{v}"),
        }
    }
}

impl From<Number> for serde_json::Value {
    fn from(num: Number) -> Self {
        match num {
            Number::Byte(n) => n.into(),
            Number::Short(n) => n.into(),
            Number::Int(n) => n.into(),
            Number::Long(n) => n.into(),
            Number::Float(n) => n.into(),
            Number::Double(n) => n.into(),
        }
    }
}

/// 从 [`serde_json::Value`] 无效转换为 [`Number`] 时返回的错误结构体。
pub struct FromJsonValueError;

impl TryFrom<&serde_json::Value> for Number {
    type Error = FromJsonValueError;

    fn try_from(num: &serde_json::Value) -> Result<Self, Self::Error> {
        num.clone().try_into()
    }
}

impl TryFrom<serde_json::Value> for Number {
    type Error = FromJsonValueError;

    fn try_from(num: serde_json::Value) -> Result<Self, Self::Error> {
        match num {
            serde_json::Value::Number(n) => n.try_into(),
            _ => Err(FromJsonValueError),
        }
    }
}

impl TryFrom<serde_json::Number> for Number {
    type Error = FromJsonValueError;

    fn try_from(num: serde_json::Number) -> Result<Self, Self::Error> {
        // 先尝试把数字转换为整数。
        num.as_i64().map_or_else(
            // 尝试浮点数转换。
            || {
                num.as_f64()
                    .map_or(Err(FromJsonValueError), |f| Ok(Self::Double(f)))
            },
            // 进行整数转换。
            |n| Ok(Self::Long(n)),
        )
    }
}
