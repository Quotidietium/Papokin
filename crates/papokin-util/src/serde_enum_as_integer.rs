use num_traits::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 将数字枚举值序列化为其对应的 `i8` 表示。
///
/// # Arguments
/// * `value` - 实现了 `ToPrimitive` 的枚举值的引用。
/// * `serializer` - 要将值写入的序列化器。
///
/// # Returns
/// 枚举的序列化表示，类型为 `i8`。
///
/// # Errors
///若该枚举无法转换为 `i8`，则返回错误。
pub fn serialize<S: Serializer, V: ToPrimitive>(
    value: &V,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let value = value
        .to_i8()
        .ok_or_else(|| serde::ser::Error::custom("Invalid enum value"))?;
    value.serialize(serializer)
}

/// 从 `i8` 表示反序列化一个数值枚举值。
///
/// # Arguments
/// * `deserializer` - 从中读取值的反序列化器。
///
/// # Returns
/// 由 `i8` 重建的枚举值。
///
/// # Errors
///若该值无法转换为目标枚举类型，则返回错误。
pub fn deserialize<'de, D: Deserializer<'de>, V: FromPrimitive>(
    deserializer: D,
) -> Result<V, D::Error> {
    let value = Deserialize::deserialize(deserializer)?;
    V::from_i8(value).ok_or_else(|| serde::de::Error::custom("Invalid enum value"))
}
