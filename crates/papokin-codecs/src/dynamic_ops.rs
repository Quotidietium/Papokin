use crate::Number;
use crate::data_result::DataResult;
use crate::list_builder::{ListBuilder, new_list_builder_impl};
use crate::map_like::MapLike;
use crate::struct_builder::ResultStructBuilder;
use std::{collections::HashMap, fmt::Display};

/// 为某个 create_*number* 函数生成默认实现。
macro_rules! create_number_impl {
    ($name:ident, $ty:ty, $number_ty:ident, $func:ident) => {
        ///返回泛型在此 `DynamicOps` 下的表示方式
        #[doc = concat!("`", stringify!($name), "`")]
        /// 在 Java 中（等价于
        #[doc = concat!("[`", stringify!($ty), "`])")]
        /// 由该 `DynamicOps` 表示。
        fn $func(&self, data: $ty) -> Self::Value {
            self.create_number(Number::$number_ty(data))
        }
    };
}

/// 一个宏，为 `DynamicOps` 中的 get_... 函数提供默认实现。
///
/// 这些函数包括：
/// - [`DynamicOps::get_byte_buffer`]（在目标表达式（通常为 self）之前放置 `box`）
/// - [`DynamicOps::get_int_list`]
/// - [`DynamicOps::get_long_list`]
#[macro_export]
macro_rules! impl_get_list {
    ($target:expr, $input:expr, $ty:literal) => {
        $target.get_iter($input).flat_map(|iter| {
            // 我们希望迭代器中的所有元素都是数字。
            iter.map(|e| $target.get_number(&e).into_result().map(Into::into))
                .collect::<Option<Vec<_>>>()
                .map_or_else(
                    || DataResult::new_error(concat!("Some elements are not ", $ty)),
                    DataResult::new_success,
                )
        })
    };
}

/// 一个描述读取和写入特定格式（如 NBT 或 JSON）方法的 trait。
/// 此 trait 的 `Value` 是可用于以该格式表示任意内容的类型。
pub trait DynamicOps {
    type Value: PartialEq + Display + Clone;
    type StructBuilder: ResultStructBuilder<Value = Self::Value>;

    ///返回空值在此 `DynamicOps` 下的表示方式。
    fn empty(&self) -> Self::Value;

    ///返回空列表在此 `DynamicOps` 下的表示方式。
    fn empty_list(&self) -> Self::Value {
        self.create_list(vec![])
    }

    ///返回空映射在此 `DynamicOps` 下的表示方式。
    fn empty_map(&self) -> Self::Value {
        self.create_map(HashMap::new())
    }

    ///返回泛型数字在此 `DynamicOps` 下的表示方式。
    fn create_number(&self, n: Number) -> Self::Value;

    create_number_impl!(byte, i8, Byte, create_byte);
    create_number_impl!(short, i16, Short, create_short);
    create_number_impl!(int, i32, Int, create_int);
    create_number_impl!(long, i64, Long, create_long);
    create_number_impl!(float, f32, Float, create_float);
    create_number_impl!(double, f64, Double, create_double);

    ///返回布尔值在此 `DynamicOps` 下的表示方式。
    fn create_bool(&self, data: bool) -> Self::Value;

    ///返回字符串在此 `DynamicOps` 下的表示方式。
    fn create_string(&self, data: &str) -> Self::Value;

    ///返回列表在此 `DynamicOps` 下的表示方式。
    fn create_list<I>(&self, values: I) -> Self::Value
    where
        I: IntoIterator<Item = Self::Value>;

    ///返回映射在此 `DynamicOps` 下的表示方式。
    fn create_map<I>(&self, entries: I) -> Self::Value
    where
        I: IntoIterator<Item = (Self::Value, Self::Value)>;

    /// 尝试获取此 `DynamicOps` 所表示的 `bool`。
    fn get_bool(&self, input: &Self::Value) -> DataResult<bool>;

    /// 尝试获取此 `DynamicOps` 所表示的数字。
    fn get_number(&self, input: &Self::Value) -> DataResult<Number>;

    /// 尝试获取此 `DynamicOps` 所表示的字符串。
    fn get_string(&self, input: &Self::Value) -> DataResult<String>;

    /// 从此 `DynamicOps` 所表示的映射中获取键值对的 [`Iterator`]。
    /// 这仅适用于类映射的值。
    fn get_map_iter<'a>(
        &'a self,
        input: &'a Self::Value,
    ) -> DataResult<impl Iterator<Item = (Self::Value, &'a Self::Value)> + 'a>;

    /// 尝试为此 `DynamicOps` 表示的映射获取 [`MapLike`]。
    fn get_map<'a>(
        &self,
        input: &'a Self::Value,
    ) -> DataResult<impl MapLike<Value = Self::Value> + 'a>;

    /// 从此 `DynamicOps` 所表示的泛型值获取一个 [`Iterator`]。
    /// 这等效于 DFU 的 `getStream()` 函数，且仅适用于类列表的值。
    fn get_iter(&self, input: Self::Value) -> DataResult<impl Iterator<Item = Self::Value>>;

    /// 从此 `DynamicOps` 表示的泛型值中获取 `Box<[u8]>`（字节缓冲区）。
    /// 这等效于 DFU 的 `getByteBuffer()` 函数。
    fn get_byte_list(&self, input: Self::Value) -> DataResult<Vec<i8>> {
        impl_get_list!(self, input, "bytes")
    }

    /// 使用 [`Vec<u8>`] 创建可由此 `DynamicOps` 表示的字节缓冲区。
    fn create_byte_list(&self, vec: Vec<i8>) -> Self::Value {
        self.create_list(vec.into_iter().map(|b| self.create_byte(b)))
    }

    /// 从此 `DynamicOps` 表示的泛型值中获取 [`Vec<i32>`]（`int` 列表）。
    /// 这等效于 DFU 的 `getIntStream()` 函数。
    fn get_int_list(&self, input: Self::Value) -> DataResult<Vec<i32>> {
        impl_get_list!(self, input, "ints")
    }

    /// 创建一个可由该 `DynamicOps` 表示的 `int` 列表（[`Vec<i32>`]）。
    fn create_int_list(&self, vec: Vec<i32>) -> Self::Value {
        self.create_list(vec.into_iter().map(|i| self.create_int(i)))
    }

    /// 从此 `DynamicOps` 表示的泛型值中获取 [`Vec<i64>`]（`long` 列表）。
    /// 这等效于 DFU 的 `getLongStream()` 函数。
    fn get_long_list(&self, input: Self::Value) -> DataResult<Vec<i64>> {
        impl_get_list!(self, input, "longs")
    }

    /// 创建可由此 `DynamicOps` 表示的 `long` 列表（[`Vec<i64>`]）。
    fn create_long_list(&self, vec: Vec<i64>) -> Self::Value {
        self.create_list(vec.into_iter().map(|l| self.create_long(l)))
    }

    /// 将此 `DynamicOps` 表示的值合并到此 `DynamicOps` 表示的列表中。
    /// 仅当 `list` 是实际的列表时，这才有效。
    fn merge_into_list(&self, list: Self::Value, value: Self::Value) -> DataResult<Self::Value>;

    /// 将此 `DynamicOps` 表示的值列表合并到另一个此类列表中。
    /// 仅当 `list` 是实际的列表时，这才有效。
    fn merge_values_into_list<I>(&self, list: Self::Value, values: I) -> DataResult<Self::Value>
    where
        I: IntoIterator<Item = Self::Value>,
    {
        let mut result = DataResult::new_success(list);

        for value in values {
            result = result.flat_map(|list_value| self.merge_into_list(list_value, value));
        }

        result
    }

    /// 将一个键值对（均由此 `DynamicOps` 表示）添加到同样由此 `DynamicOps` 表示的映射中，
    /// 返回新地图。仅当 `map` 是实际地图或为空时才有效。
    fn merge_into_map(
        &self,
        map: Self::Value,
        key: Self::Value,
        value: Self::Value,
    ) -> DataResult<Self::Value>
    where
        Self::Value: Clone;

    /// 将此 `DynamicOps` 表示的映射合并到另一个此类映射中，并返回新映射。
    /// 仅当 `map` 是实际的映射或为空时，这才有效。
    fn merge_entries_into_map<I>(&self, map: Self::Value, entries: I) -> DataResult<Self::Value>
    where
        I: IntoIterator<Item = (Self::Value, Self::Value)>,
        Self::Value: Clone,
    {
        let mut result = DataResult::new_success(map);

        for (key, value) in entries {
            result = result.flat_map(|list_value| self.merge_into_map(list_value, key, value));
        }

        result
    }

    /// 将此 `DynamicOps` 表示的 [`MapLike`] 合并到另一个此类映射中，并返回新映射。
    /// 仅当 `map` 是实际的映射或为空时，这才有效。
    fn merge_map_like_into_map<M>(
        &self,
        map: Self::Value,
        other_map_like: M,
    ) -> DataResult<Self::Value>
    where
        M: MapLike<Value = Self::Value>,
        Self::Value: Clone,
    {
        let mut result = DataResult::new_success(map);

        for (key, value) in other_map_like.iter() {
            result =
                result.flat_map(|list_value| self.merge_into_map(list_value, key, value.clone()));
        }

        result
    }

    /// 将此 `DynamicOps` 表示的值合并为基本类型。
    fn merge_into_primitive(
        &self,
        prefix: Self::Value,
        value: Self::Value,
    ) -> DataResult<Self::Value>
    where
        <Self as DynamicOps>::Value: PartialEq,
    {
        if prefix == self.empty() {
            DataResult::new_success(value)
        } else {
            DataResult::new_error(format!(
                "Do not know how to append a primitive value {value} to {prefix}"
            ))
        }
    }

    /// 尝试使用键从此 `DynamicOps` 表示的值中移除某些内容。
    /// 成功时返回新值，否则返回其自身。
    fn remove(&self, input: Self::Value, key: &str) -> Self::Value;

    /// 尝试使用键从此 `DynamicOps` 表示的值中获取一个值。
    /// 仅对可以作为 [`MapLike`] 查看的值有效。
    fn get_element<'a>(&'a self, input: &'a Self::Value, key: &str) -> DataResult<&'a Self::Value> {
        self.get_element_generic(input, &self.create_string(key))
    }

    /// 尝试使用同样由此 `DynamicOps` 表示的键，从此 `DynamicOps` 表示的值中获取一个值。
    fn get_element_generic<'a>(
        &'a self,
        input: &'a Self::Value,
        key: &Self::Value,
    ) -> DataResult<&'a Self::Value>
where {
        self.get_map_iter(input).flat_map(|mut iter| {
            iter.find(|(k, _)| k == key).map_or_else(
                || DataResult::new_error(format!("No element {key} in the map")),
                |(_, v)| DataResult::new_success(v),
            )
        })
    }

    /// 尝试将此 `DynamicOps` 表示的一个值按给定键写入同样由此 `DynamicOps` 表示的映射中。
    /// - 若此操作成功，则返回新的映射值。
    /// - 否则直接返回 `input`。
    fn set_element(&self, input: &Self::Value, key: &str, value: Self::Value) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.merge_into_map(input.clone(), self.create_string(key), value)
            .into_result()
            .unwrap_or(input.clone())
    }

    /// 尝试用……更新此 `DynamicOps` 表示的映射中由此 `DynamicOps` 表示的一个值，
    /// 一个键和一个映射函数（`f`），其返回值将成为指定键的新值。
    /// - 如果操作成功，则返回新操作后的映射。
    /// - 否则直接返回 `input`。
    fn update_element<F>(&self, input: &Self::Value, key: &str, f: F) -> Self::Value
    where
        F: FnOnce(&Self::Value) -> Self::Value,
    {
        self.get_element(input, key)
            .map(|v| self.set_element(input, key, f(v)))
            .into_result()
            .unwrap_or(input.clone())
    }

    /// 尝试用……更新此 `DynamicOps` 表示的映射中由此 `DynamicOps` 表示的一个值，
    /// 一个同样由该 `DynamicOps` 表示的键，以及一个映射函数（`f`），其返回值将成为新键的值。
    /// - 如果操作成功，则返回新操作后的映射。
    /// - 否则直接返回 `input`。
    fn update_element_generic<F>(&self, input: &Self::Value, key: &Self::Value, f: F) -> Self::Value
    where
        F: FnOnce(&Self::Value) -> Self::Value,
    {
        self.get_element_generic(input, key)
            .flat_map(|v| self.merge_into_map(input.clone(), key.clone(), f(v)))
            .into_result()
            .unwrap_or(input.clone())
    }

    /// 将由此 `DynamicOps` 表示的值转换为由另一个 `DynamicOps` 表示的值。
    fn convert_to<U>(&self, out_ops: &impl DynamicOps<Value = U>, input: Self::Value) -> U;

    /// 将由此 `DynamicOps` 表示的列表转换为由另一个 `DynamicOps` 表示的列表。
    fn convert_list<U>(&self, out_ops: &impl DynamicOps<Value = U>, input: Self::Value) -> U {
        out_ops.create_list(
            self.get_iter(input)
                .into_result()
                .into_iter()
                .flatten()
                .map(|v| self.convert_to(out_ops, v)),
        )
    }

    /// 将由此 `DynamicOps` 表示的映射转换为由另一个 `DynamicOps` 表示的映射。
    fn convert_map<U>(&self, out_ops: &impl DynamicOps<Value = U>, input: Self::Value) -> U {
        out_ops.create_map(
            self.get_map_iter(&input)
                .into_result()
                .into_iter()
                .flatten()
                .map(|(k, v)| {
                    (
                        self.convert_to(out_ops, k),
                        self.convert_to(out_ops, v.clone()),
                    )
                }),
        )
    }

    ///为该 `DynamicOps` 返回一个 [`ListBuilder`]。
    fn list_builder(&'static self) -> impl ListBuilder<Value = Self::Value>
    where
        Self: Sized,
    {
        new_list_builder_impl(self)
    }

    ///为该 `DynamicOps` 返回一个 [`MapBuilder`]。
    fn map_builder(&'static self) -> Self::StructBuilder;
}
