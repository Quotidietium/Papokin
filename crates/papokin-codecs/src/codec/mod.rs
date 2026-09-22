pub mod either;
pub mod list;
pub mod map;
pub mod optional_field;
pub(crate) mod primitive;

use crate::codec::optional_field::OptionalFieldDecode;
use crate::map_like::MapLike;
use crate::struct_builder::StructBuilder;
use crate::{DataResult, DynamicOps};

/// 一个 trait，表示可被 [`DynamicOps`] 编码为其格式的对象。
pub trait Encode {
    /// 将此值编码为由所提供的 [`DynamicOps`] 表示的值
    /// 具有所提供的前缀。
    fn encode<O: DynamicOps>(&self, ops: &'static O, prefix: O::Value) -> DataResult<O::Value>;

    /// 将此值编码为由所提供的 [`DynamicOps`] 表示的值，不带前缀。
    fn encode_start<O: DynamicOps>(&self, ops: &'static O) -> DataResult<O::Value> {
        self.encode(ops, ops.empty())
    }
}

/// 一个 trait，表示可以作为字段以所提供名称添加到 [`MapLike`] 的对象。
pub trait FieldEncode {
    /// 通过添加一个字段将此值编码为映射，该字段：
    /// - 键是字段的 `name`。
    /// - 值是所提供的 [`DynamicOps`] 所表示的编码值。
    fn encode_field<O: DynamicOps, B: StructBuilder<Value = O::Value>>(
        &self,
        name: &'static str,
        ops: &'static O,
        prefix: B,
    ) -> B;

    /// 通过添加一个带默认值的字段将此值编码为映射，该字段：
    /// - 键是字段的 `name`。
    /// - 值是所提供的 [`DynamicOps`] 所表示的编码值。
    ///
    /// 当 `default` == `*self` 时，该字段可能不会被编码。
    fn encode_defaulted_field<O: DynamicOps, B: StructBuilder<Value = O::Value>>(
        &self,
        name: &'static str,
        ops: &'static O,
        prefix: B,
        default: Self,
    ) -> B
    where
        Self: PartialEq;
}

impl<T: Encode> FieldEncode for T {
    fn encode_field<O: DynamicOps, B: StructBuilder<Value = O::Value>>(
        &self,
        name: &'static str,
        ops: &'static O,
        prefix: B,
    ) -> B {
        prefix.add_string_key_value_result(name, self.encode_start(ops))
    }

    fn encode_defaulted_field<O: DynamicOps, B: StructBuilder<Value = O::Value>>(
        &self,
        name: &'static str,
        ops: &'static O,
        prefix: B,
        default: Self,
    ) -> B
    where
        Self: PartialEq,
    {
        if default == *self {
            prefix.add_string_key_value_result(name, self.encode_start(ops))
        } else {
            prefix
        }
    }
}

/// 一个 trait，表示可以从 [`DynamicOps`] 所表示的值解码的对象。
pub trait Decode: Sized {
    /// 从由提供的 [`DynamicOps`] 所表示的值中解码此类型的值，
    /// 连同剩余数据。
    fn decode<O: DynamicOps>(input: O::Value, ops: &'static O) -> DataResult<(Self, O::Value)>;

    /// 从由提供的 [`DynamicOps`] 所表示的值中解码此类型的值，
    /// 而不提供任何其他数据。
    fn parse<O: DynamicOps>(input: O::Value, ops: &'static O) -> DataResult<Self> {
        Self::decode(input, ops).map(|(r, _)| r)
    }
}

/// 一个 trait，表示可以从 [`MapLike`] 中具有所提供名称的字段解码的对象。
pub trait FieldDecode: Sized {
    /// 通过解码其中一个字段，从映射中解码此类型的值，该字段：
    /// - 键是字段的 `name`。
    /// - 值是 [`DynamicsOps`] 所表示的、待解码的值。
    fn decode_field<O: DynamicOps>(
        name: &'static str,
        input: &impl MapLike<Value = O::Value>,
        ops: &'static impl DynamicOps<Value = O::Value>,
    ) -> DataResult<Self>;

    /// 通过解码其中一个带默认值的字段，从映射中解码此类型的值，该字段：
    /// - 键是字段的 `name`。
    /// - 值是 [`DynamicsOps`] 所表示的、待解码的值。
    ///
    /// 若某个值无法解码，则返回 `default` 值。
    /// 此方法有一个额外的 `lenient` 参数。若为 `true`，错误
    /// 在尝试解码显式值时发生，之后会改为解码默认值。
    fn decode_defaulted_field<O: DynamicOps>(
        name: &'static str,
        input: &impl MapLike<Value = O::Value>,
        ops: &'static impl DynamicOps<Value = O::Value>,
        default: Self,
        lenient: bool,
    ) -> DataResult<Self>;
}

impl<T: Decode> FieldDecode for T {
    fn decode_field<O: DynamicOps>(
        name: &'static str,
        input: &impl MapLike<Value = O::Value>,
        ops: &'static impl DynamicOps<Value = O::Value>,
    ) -> DataResult<Self> {
        input.get_str(name).map_or_else(
            || DataResult::new_error(format!("No key {name} in map")),
            |v| Self::parse(v.clone(), ops),
        )
    }

    fn decode_defaulted_field<O: DynamicOps>(
        name: &'static str,
        input: &impl MapLike<Value = O::Value>,
        ops: &'static impl DynamicOps<Value = O::Value>,
        default: Self,
        lenient: bool,
    ) -> DataResult<Self> {
        let decoded_option = Option::decode_optional_field::<O>(name, input, ops, lenient);
        decoded_option.map(|o| o.unwrap_or(default))
    }
}

/// 通过 `backward` 提供从第一类型到第二类型的可失败/不可失败编码转换。
///
/// 你可能不需要直接使用它。
#[macro_export]
macro_rules! encode_impl {
    (infallible $second_type:ty, $backward:path) => {
        impl $crate::Encode for $second_type {
            fn encode<O: $crate::DynamicOps>(
                &self,
                ops: &'static O,
                prefix: O::Value,
            ) -> $crate::DataResult<O::Value> {
                $backward(self).encode(ops, prefix)
            }
        }
    };

    (fallible $second_type:ty, $backward:path) => {
        impl $crate::Encode for $second_type {
            fn encode<O: $crate::DynamicOps>(
                &self,
                ops: &'static O,
                prefix: O::Value,
            ) -> $crate::DataResult<O::Value> {
                $backward(self).flat_map(|m| m.encode(ops, prefix))
            }
        }
    };
}

/// 通过 `forward` 提供从第一类型到第二类型的可失败/不可失败解码转换。
///
/// 你可能不需要直接使用它。
#[macro_export]
macro_rules! decode_impl {
    (infallible $first_type:ty, $second_type:ty, $forward:path) => {
        impl $crate::Decode for $second_type {
            fn decode<O: $crate::DynamicOps>(
                input: O::Value,
                ops: &'static O,
            ) -> $crate::DataResult<(Self, O::Value)> {
                <$first_type>::decode(input, ops).map(|(s, p)| ($forward(s), p))
            }
        }
    };

    (fallible $first_type:ty, $second_type:ty, $forward:path) => {
        impl $crate::Decode for $second_type {
            fn decode<O: $crate::DynamicOps>(
                input: O::Value,
                ops: &'static O,
            ) -> $crate::DataResult<(Self, O::Value)> {
                <$first_type>::decode(input, ops).flat_map(|(s, p)| $forward(s).map(|m| (m, p)))
            }
        }
    };
}

/// 提供类似 `xmap` 的简便 `Encode` 与 `Decode` 实现，针对*第二类型*
/// 通过使用*第一类型*已有的实现。
///
/// 该宏的写法为 `xmap_codec_impl!(first => second, forward, backward)`，
/// 其中：
/// - `forward` 是不会失败的转换 `fn(first) -> second`（用于解码）。
/// - `backward` 是不会失败的转换 `fn(&second) -> first`（用于编码）。
#[macro_export]
macro_rules! xmap_codec_impl {
    ($first_type:ty => $second_type:ty, $forward:path, $backward:path) => {
        $crate::encode_impl!(infallible $second_type, $backward);
        $crate::decode_impl!(infallible $first_type, $second_type, $forward);
    };
}

/// 提供类似 `comapFlatMap` 的简便 `Encode` 与 `Decode` 实现，针对*第二类型*
/// 通过使用*第一类型*已有的实现。
///
/// 该宏的写法为 `comap_flat_map_codec_impl!(first => second, forward, backward)`，
/// 其中：
/// - `forward` 是可能失败的转换 `fn(first) -> DataResult<second>`（用于解码）。
/// - `backward` 是不会失败的转换 `fn(&second) -> first`（用于编码）。
#[macro_export]
macro_rules! comap_flat_map_codec_impl {
    ($first_type:ty => $second_type:ty, $forward:path, $backward:path) => {
        $crate::encode_impl!(infallible $second_type, $backward);
        $crate::decode_impl!(fallible $first_type, $second_type, $forward);
    };
}

/// 提供类似 `flatComapMap` 的简便 `Encode` 与 `Decode` 实现，针对*第二类型*
/// 通过使用*第一类型*已有的实现。
///
/// 该宏的写法为 `flat_comap_map_codec_impl!(first => second, forward, backward)`，
/// 其中：
/// - `forward` 是不会失败的转换 `fn(first) -> second`（用于解码）。
/// - `backward` 是可能失败的转换 `fn(&second) -> DataResult<first>`（用于编码）。
#[macro_export]
macro_rules! flat_comap_map_codec_impl {
    ($first_type:ty => $second_type:ty, $forward:path, $backward:path) => {
        $crate::encode_impl!(fallible $second_type, $backward);
        $crate::decode_impl!(infallible $first_type, $second_type, $forward);
    };
}

/// 提供类似 `flatXmap` 的简便 `Encode` 与 `Decode` 实现，针对*第二类型*
/// 通过使用*第一类型*已有的实现。
///
/// 该宏的写法为 `flat_xmap_codec_impl!(first => second, forward, backward)`，
/// 其中：
/// - `forward` 是可能失败的转换 `fn(first) -> DataResult<second>`（用于解码）。
/// - `backward` 是可能失败的转换 `fn(&second) -> DataResult<first>`（用于编码）。
#[macro_export]
macro_rules! flat_xmap_codec_impl {
    ($first_type:ty => $second_type:ty, $forward:path, $backward:path) => {
        $crate::encode_impl!(fallible $second_type, $backward);
        $crate::decode_impl!(fallible $first_type, $second_type, $forward);
    };
}
