use crate::codec::FieldEncode;
use crate::map_like::MapLike;
use crate::struct_builder::StructBuilder;
use crate::{DataResult, Decode, DynamicOps, Encode};

/// 一个 trait，表示可以作为可选字段以所提供名称添加到 [`MapLike`] 的对象。
pub trait OptionalFieldEncode {
    /// 通过添加一个可选字段将此值编码为映射，该字段：
    /// - 键是字段的 `name`。
    /// - 值是所提供的 [`DynamicOps`] 所表示的编码值。
    fn encode_optional_field<O: DynamicOps, B: StructBuilder<Value = O::Value>>(
        &self,
        name: &'static str,
        ops: &'static O,
        prefix: B,
    ) -> B;
}

impl<T> OptionalFieldEncode for Option<T>
where
    T: Encode,
{
    fn encode_optional_field<O: DynamicOps, B: StructBuilder<Value = O::Value>>(
        &self,
        name: &'static str,
        ops: &'static O,
        prefix: B,
    ) -> B {
        if let Some(value) = self {
            value.encode_field(name, ops, prefix)
        } else {
            prefix
        }
    }
}

/// 一个 trait，用于将 [`MapLike`] 的可选字段解码为
/// 实现类型的值。
///
/// 此 trait 没有 `OptionalFieldEncode` 变体；只需
/// 编码可选字段时请使用 [`FieldEncode::encode_field`]。
pub trait OptionalFieldDecode: Sized {
    /// 从映射中解码一个可选字段，类似于 [`FieldDecode::decode_field`]。
    ///
    /// 不过，此方法多了一个 `lenient` 参数。若其为 `true`，错误
    /// 解码时不会出现 `Some`，而是解码出 `None`。
    fn decode_optional_field<O: DynamicOps>(
        name: &'static str,
        input: &impl MapLike<Value = O::Value>,
        ops: &'static impl DynamicOps<Value = O::Value>,
        lenient: bool,
    ) -> DataResult<Self>;
}

impl<T> OptionalFieldDecode for Option<T>
where
    T: Decode,
{
    fn decode_optional_field<O: DynamicOps>(
        name: &'static str,
        input: &impl MapLike<Value = O::Value>,
        ops: &'static impl DynamicOps<Value = O::Value>,
        lenient: bool,
    ) -> DataResult<Self> {
        input.get_str(name).map_or_else(
            || DataResult::new_success(None),
            |value| {
                let result = T::parse(value.clone(), ops);
                if result.is_error() && lenient {
                    DataResult::new_success(None)
                } else {
                    result.map(Some)
                }
            },
        )
    }
}
