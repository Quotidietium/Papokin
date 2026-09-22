//! 为 Pumpkin 的动态序列化操作提供 NBT 支持。

use crate::compound::NbtCompound;
use crate::tag::NbtTag;
use papokin_codecs::DataResult;
use papokin_codecs::DynamicOps;
use papokin_codecs::Lifecycle;
use papokin_codecs::MapLike;
use papokin_codecs::Number;
use papokin_codecs::struct_builder::{ResultStructBuilder, StringStructBuilder, StructBuilder};
use papokin_codecs::{impl_get_list, impl_string_struct_builder, impl_struct_builder};
use std::iter::Map;
use std::vec::IntoIter;
use tracing::warn;

/// 一个 [`DynamicOps`] 实现，将值表示为 [`NbtTag`]。
pub struct NbtOps;

impl DynamicOps for NbtOps {
    type Value = NbtTag;
    type StructBuilder = NbtStructBuilder;

    fn empty(&self) -> Self::Value {
        NbtTag::End
    }

    fn create_number(&self, n: Number) -> Self::Value {
        NbtTag::Double(n.into())
    }

    fn create_byte(&self, data: i8) -> Self::Value {
        NbtTag::Byte(data)
    }

    fn create_short(&self, data: i16) -> Self::Value {
        NbtTag::Short(data)
    }

    fn create_int(&self, data: i32) -> Self::Value {
        NbtTag::Int(data)
    }

    fn create_long(&self, data: i64) -> Self::Value {
        NbtTag::Long(data)
    }

    fn create_float(&self, data: f32) -> Self::Value {
        NbtTag::Float(data)
    }

    fn create_double(&self, data: f64) -> Self::Value {
        NbtTag::Double(data)
    }

    fn create_bool(&self, data: bool) -> Self::Value {
        NbtTag::Byte(data.into())
    }

    fn create_string(&self, data: &str) -> Self::Value {
        NbtTag::String(data.into())
    }

    fn create_list<I>(&self, values: I) -> Self::Value
    where
        I: IntoIterator<Item = Self::Value>,
    {
        ListCollector::new_collector().accept_all(values).result()
    }

    fn create_map<I>(&self, entries: I) -> Self::Value
    where
        I: IntoIterator<Item = (Self::Value, Self::Value)>,
    {
        let mut compound = NbtCompound::new();
        for (k, v) in entries {
            if let Some(key) = k.extract_string() {
                compound.put(key, v);
            } else {
                // Minecraft 的实现只是直接使用键标签的字符串表示，
                // 但那大概并非有意如此使用，所以我们干脆
                // 记录一条警告。
                warn!("创建 NBT 复合标签的键标签无效：{k}");
            }
        }
        compound.into()
    }

    fn get_bool(&self, input: &Self::Value) -> DataResult<bool> {
        self.get_number(input).map(|n| f64::from(n) != 0.0)
    }

    fn get_number(&self, input: &Self::Value) -> DataResult<Number> {
        match input {
            NbtTag::Byte(b) => DataResult::new_success(Number::Byte(*b)),
            NbtTag::Short(s) => DataResult::new_success(Number::Short(*s)),
            NbtTag::Int(i) => DataResult::new_success(Number::Int(*i)),
            NbtTag::Long(l) => DataResult::new_success(Number::Long(*l)),
            NbtTag::Float(f) => DataResult::new_success(Number::Float(*f)),
            NbtTag::Double(d) => DataResult::new_success(Number::Double(*d)),

            _ => DataResult::new_error("Not a number".to_string()),
        }
    }

    fn get_string(&self, input: &Self::Value) -> DataResult<String> {
        input.extract_string().map_or_else(
            || DataResult::new_error("Not a string".to_string()),
            |s| DataResult::new_success(s.to_string()),
        )
    }

    fn get_map_iter<'a>(
        &'a self,
        input: &'a Self::Value,
    ) -> DataResult<impl Iterator<Item = (Self::Value, &'a Self::Value)> + 'a> {
        if let NbtTag::Compound(compound) = input {
            DataResult::new_success(
                compound
                    .child_tags
                    .iter()
                    .map(|(k, v)| (self.create_string(k), v)),
            )
        } else {
            DataResult::new_error(format!("Not a map: {input}"))
        }
    }

    fn get_map<'a>(
        &self,
        input: &'a Self::Value,
    ) -> DataResult<impl MapLike<Value = Self::Value> + 'a> {
        if let NbtTag::Compound(compound) = input {
            DataResult::new_success(NbtMapLike { compound })
        } else {
            DataResult::new_error(format!("Not a map: {input}"))
        }
    }

    fn get_iter(&self, input: Self::Value) -> DataResult<impl Iterator<Item = Self::Value>> {
        match input {
            NbtTag::List(l) => {
                // 检查该列表的类型。
                // 如果列表包含复合标签，尝试将其解包。
                if let Some(NbtTag::Compound(_)) = l.first() {
                    DataResult::new_success(NbtIter::CompoundList(l.into_iter().map(|c| {
                        if let NbtTag::Compound(compound) = c {
                            Self::try_unwrap(compound)
                        } else {
                            c
                        }
                    })))
                } else {
                    DataResult::new_success(NbtIter::List(l.into_iter()))
                }
            }

            NbtTag::ByteArray(b) => DataResult::new_success(NbtIter::ByteArray(
                b.into_iter().map(|b| Self.create_byte(b)),
            )),
            NbtTag::IntArray(i) => DataResult::new_success(NbtIter::IntArray(
                i.into_iter().map(|i| Self.create_int(i)),
            )),
            NbtTag::LongArray(l) => DataResult::new_success(NbtIter::LongArray(
                l.into_iter().map(|l| Self.create_long(l)),
            )),

            _ => DataResult::new_error(format!("Not a list: {input}")),
        }
    }

    fn get_byte_list(&self, input: Self::Value) -> DataResult<Vec<i8>> {
        if let NbtTag::ByteArray(b) = input {
            DataResult::new_success(b.into())
        } else {
            impl_get_list!(self, input, "bytes")
        }
    }

    fn create_byte_list(&self, buffer: Vec<i8>) -> Self::Value {
        NbtTag::ByteArray(buffer.into())
    }

    fn get_int_list(&self, input: Self::Value) -> DataResult<Vec<i32>> {
        if let NbtTag::IntArray(i) = input {
            DataResult::new_success(i)
        } else {
            impl_get_list!(self, input, "ints")
        }
    }

    fn create_int_list(&self, list: Vec<i32>) -> Self::Value {
        NbtTag::IntArray(list)
    }

    fn get_long_list(&self, input: Self::Value) -> DataResult<Vec<i64>> {
        if let NbtTag::LongArray(i) = input {
            DataResult::new_success(i)
        } else {
            impl_get_list!(self, input, "longs")
        }
    }

    fn create_long_list(&self, list: Vec<i64>) -> Self::Value {
        NbtTag::LongArray(list)
    }

    fn merge_into_list(&self, list: Self::Value, value: Self::Value) -> DataResult<Self::Value> {
        ListCollector::new(list.clone()).map_or_else(
            || DataResult::new_partial_error("Not a list".to_string(), list),
            |c| DataResult::new_success(c.accept(value).result()),
        )
    }

    fn merge_values_into_list<I>(&self, list: Self::Value, values: I) -> DataResult<Self::Value>
    where
        I: IntoIterator<Item = Self::Value>,
    {
        ListCollector::new(list.clone()).map_or_else(
            || DataResult::new_partial_error("Not a list".to_string(), list),
            |c| DataResult::new_success(c.accept_all(values).result()),
        )
    }

    fn merge_into_map(
        &self,
        map: Self::Value,
        key: Self::Value,
        value: Self::Value,
    ) -> DataResult<Self::Value>
    where
        Self::Value: Clone,
    {
        if !matches!(map, NbtTag::Compound(_) | NbtTag::End) {
            DataResult::new_partial_error(format!("Not a map: {map}"), map)
        } else if !matches!(key, NbtTag::String(_)) {
            DataResult::new_partial_error(format!("Key is not a string: {key}"), map)
        } else {
            let mut compound = if let NbtTag::Compound(c) = map {
                c
            } else {
                NbtCompound::new()
            };
            key.extract_string().map_or_else(
                || DataResult::new_error(format!("Key is not a string: {key}")),
                |key_str| {
                    compound.put(key_str, value);
                    DataResult::new_success(compound.into())
                },
            )
        }
    }

    fn merge_map_like_into_map<M>(
        &self,
        map: Self::Value,
        other_map_like: M,
    ) -> DataResult<Self::Value>
    where
        M: MapLike<Value = Self::Value>,
        Self::Value: Clone,
    {
        if matches!(map, NbtTag::Compound(_) | NbtTag::End) {
            let mut compound = if let NbtTag::Compound(c) = map {
                c
            } else {
                NbtCompound::default()
            };
            let mut failed = vec![];
            other_map_like.iter().for_each(|(k, v)| {
                if let NbtTag::String(key) = k {
                    compound.put(&key, v.clone());
                } else {
                    failed.push((k, v));
                }
            });
            if failed.is_empty() {
                DataResult::new_success(compound.into())
            } else {
                DataResult::new_partial_error(
                    format!("Some keys are not strings: {failed:?}"),
                    NbtTag::Compound(compound),
                )
            }
        } else {
            DataResult::new_partial_error(format!("Not a map: {map}"), map)
        }
    }

    fn remove(&self, input: Self::Value, key: &str) -> Self::Value {
        if let NbtTag::Compound(compound) = input {
            // 尝试移除键与 `key` 匹配的所有条目。
            NbtTag::Compound(
                compound
                    .child_tags
                    .into_iter()
                    .filter(|s| s.0.as_ref() != key)
                    .collect(),
            )
        } else {
            input
        }
    }

    fn convert_to<U>(&self, out_ops: &impl DynamicOps<Value = U>, input: Self::Value) -> U {
        match input {
            NbtTag::End => out_ops.empty(),
            NbtTag::Byte(b) => out_ops.create_byte(b),
            NbtTag::Short(s) => out_ops.create_short(s),
            NbtTag::Int(i) => out_ops.create_int(i),
            NbtTag::Long(l) => out_ops.create_long(l),
            NbtTag::Float(f) => out_ops.create_float(f),
            NbtTag::Double(d) => out_ops.create_double(d),
            NbtTag::ByteArray(b) => out_ops.create_byte_list(b.to_vec()),
            NbtTag::String(s) => out_ops.create_string(&s),
            NbtTag::List(_) => self.convert_list(out_ops, input),
            NbtTag::Compound(_) => self.convert_map(out_ops, input),
            NbtTag::IntArray(i) => out_ops.create_int_list(i),
            NbtTag::LongArray(l) => out_ops.create_long_list(l),
        }
    }

    fn map_builder(&'static self) -> Self::StructBuilder {
        NbtStructBuilder {
            builder: DataResult::new_success_with_lifecycle(
                NbtTag::Compound(NbtCompound::new()),
                Lifecycle::Stable,
            ),
        }
    }
}

impl NbtOps {
    /// 尝试解包一个 [`NbtCompound`]。
    ///
    /// 若 `compound` 仅有一个键为空（`""`）的元素，则返回该元素。
    /// 否则，直接返回一个包含 `compound` 的新 [`NbtTag::Compound`]。
    fn try_unwrap(mut compound: NbtCompound) -> NbtTag {
        if compound.child_tags.len() == 1 && compound.has("") {
            // 移除该元素以取得包含标签的所有权。
            compound
                .child_tags
                .remove("")
                .unwrap_or_else(|| NbtTag::from(compound))
        } else {
            NbtTag::from(compound)
        }
    }
}

/// NBT 元素迭代器的单个具体类型。
enum NbtIter {
    List(IntoIter<NbtTag>),
    CompoundList(Map<IntoIter<NbtTag>, fn(NbtTag) -> NbtTag>),
    ByteArray(Map<IntoIter<i8>, fn(i8) -> NbtTag>),
    IntArray(Map<IntoIter<i32>, fn(i32) -> NbtTag>),
    LongArray(Map<IntoIter<i64>, fn(i64) -> NbtTag>),
}

impl Iterator for NbtIter {
    type Item = NbtTag;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::List(iter) => iter.next(),
            Self::CompoundList(iter) => iter.next(),
            Self::ByteArray(iter) => iter.next(),
            Self::IntArray(iter) => iter.next(),
            Self::LongArray(iter) => iter.next(),
        }
    }
}

/// 针对 NBT 对象的 [`MapLike`] 实现。
/// 生命周期与被引用的映射相同。
struct NbtMapLike<'a> {
    compound: &'a NbtCompound,
}

impl MapLike for NbtMapLike<'_> {
    type Value = NbtTag;

    fn get(&self, key: &Self::Value) -> Option<&Self::Value> {
        key.extract_string().and_then(|s| self.get_str(s))
    }

    fn get_str(&self, key: &str) -> Option<&Self::Value> {
        self.compound.get(key)
    }

    fn iter(&self) -> impl Iterator<Item = (Self::Value, &Self::Value)> + '_ {
        self.compound
            .child_tags
            .iter()
            .map(|(k, v)| (NbtTag::String(k.clone()), v))
    }
}

/// 为 [`NbtOps`] 动态编解码器实现构建 NBT 复合标签。
pub struct NbtStructBuilder {
    builder: DataResult<NbtTag>,
}

impl ResultStructBuilder for NbtStructBuilder {
    type Result = NbtTag;

    fn build_with_builder(
        self,
        builder: Self::Result,
        prefix: Self::Value,
    ) -> DataResult<Self::Value> {
        match prefix {
            NbtTag::End => DataResult::new_success(builder),
            NbtTag::Compound(mut compound) => {
                match builder {
                    NbtTag::Compound(builder_compound) => {
                        for (k, v) in builder_compound {
                            compound.put(&k, v);
                        }
                    }
                    // 这本不该发生，但以防万一。
                    _ => {
                        return DataResult::new_error(format!(
                            "Expected compound in builder, found {builder}"
                        ));
                    }
                }
                DataResult::new_success(compound.into())
            }
            _ => DataResult::new_partial_error(format!("Prefix is not a map: {prefix}"), prefix),
        }
    }
}

impl StructBuilder for NbtStructBuilder {
    type Value = NbtTag;

    impl_struct_builder!(builder);
    impl_string_struct_builder!(builder, NbtOps);
}

impl StringStructBuilder for NbtStructBuilder {
    fn append(&self, key: &str, value: Self::Value, builder: Self::Result) -> Self::Result {
        if let NbtTag::Compound(mut compound) = builder {
            compound.put(key, value);
            compound.into()
        } else {
            builder
        }
    }
}

// 列表收集器

/// NBT 列表的收集器对象。
///
/// 不应使用此对象的各个变体，因为那属于实现细节。
enum ListCollector {
    Generic(InnerGenericListCollector),

    Byte(InnerByteListCollector),
    Int(InnerIntListCollector),
    Long(InnerLongListCollector),
}

impl ListCollector {
    /// 创建新的 [`ListCollector`]。
    ///
    /// 此方法只为 [`NbtTag::End`] 和所有列表类 [`NbtTag`] 返回实际的收集器。
    fn new(tag: NbtTag) -> Option<Self> {
        match tag {
            NbtTag::End => Some(Self::new_collector()),

            NbtTag::List(_) | NbtTag::ByteArray(_) | NbtTag::IntArray(_) | NbtTag::LongArray(_) => {
                // 尝试获取标签的长度。
                let len = match &tag {
                    NbtTag::List(list) => list.len(),

                    NbtTag::ByteArray(list) => list.len(),
                    NbtTag::IntArray(list) => list.len(),
                    NbtTag::LongArray(list) => list.len(),

                    _ => return None,
                };

                if len == 0 {
                    return Some(Self::new_collector());
                }

                // 从这里开始，我们知道列表不为空。
                match tag {
                    NbtTag::List(list) => Some(Self::Generic(InnerGenericListCollector::new(list))),
                    NbtTag::ByteArray(list) => {
                        Some(Self::Byte(InnerByteListCollector::new(list.into())))
                    }
                    NbtTag::IntArray(list) => Some(Self::Int(InnerIntListCollector::new(list))),
                    NbtTag::LongArray(list) => Some(Self::Long(InnerLongListCollector::new(list))),

                    _ => None,
                }
            }

            _ => None,
        }
    }

    /// 创建新的初始收集器。
    /// [`NbtTag`] 可以直接添加到此收集器中，无需担心任何类型问题。
    const fn new_collector() -> Self {
        Self::Generic(InnerGenericListCollector {
            result: NbtTag::List(vec![]),
        })
    }

    /// 接受一个 [`NbtTag`]。
    fn accept(self, tag: NbtTag) -> Self {
        match self {
            Self::Generic(c) => c.accept(tag),
            Self::Byte(c) => c.accept(tag),
            Self::Int(c) => c.accept(tag),
            Self::Long(c) => c.accept(tag),
        }
    }

    /// 接受所提供列表中的所有 [`NbtTag`]。
    fn accept_all(self, tags: impl IntoIterator<Item = NbtTag>) -> Self {
        let mut collector = self;
        for tag in tags {
            collector = collector.accept(tag);
        }
        collector
    }

    /// 提供最终结果。
    fn result(self) -> NbtTag {
        match self {
            Self::Generic(c) => c.result(),
            Self::Byte(c) => c.result(),
            Self::Int(c) => c.result(),
            Self::Long(c) => c.result(),
        }
    }
}

/// 存储在对应 [`ListCollector`] 枚举之一的“内部”列表收集器。
trait InnerListCollector {
    fn accept(self, tag: NbtTag) -> ListCollector
    where
        Self: Sized;

    fn result(self) -> NbtTag;
}

/// 针对（任意类型的）通用列表的 [`InnerListCollector`] 实现。
struct InnerGenericListCollector {
    result: NbtTag,
}

impl InnerListCollector for InnerGenericListCollector {
    fn accept(mut self, tag: NbtTag) -> ListCollector
    where
        Self: Sized,
    {
        if let NbtTag::List(list) = &mut self.result {
            list.push(tag);
        }
        ListCollector::Generic(self)
    }

    fn result(self) -> NbtTag {
        self.result
    }
}

impl From<InnerByteListCollector> for InnerGenericListCollector {
    fn from(value: InnerByteListCollector) -> Self {
        Self {
            result: NbtTag::List(value.list.into_iter().map(NbtTag::Byte).collect()),
        }
    }
}

impl InnerGenericListCollector {
    const fn new(list: Vec<NbtTag>) -> Self {
        Self {
            result: NbtTag::List(list),
        }
    }
}

/// 专门针对 [`NbtTag::ByteArray`] 的 [`InnerListCollector`] 实现。
struct InnerByteListCollector {
    list: Vec<i8>,
}

impl InnerListCollector for InnerByteListCollector {
    fn accept(mut self, tag: NbtTag) -> ListCollector
    where
        Self: Sized,
    {
        if let NbtTag::Byte(byte) = tag {
            self.list.push(byte);
            ListCollector::Byte(self)
        } else {
            <Self as Into<InnerGenericListCollector>>::into(self).accept(tag)
        }
    }

    fn result(self) -> NbtTag {
        NbtTag::ByteArray(self.list.into())
    }
}

impl InnerByteListCollector {
    const fn new(list: Vec<i8>) -> Self {
        Self { list }
    }
}

macro_rules! add_inner_specific_array_collector_impl {
    ($name:ident, $single_type:ident, $array_type:ident, $ty:ty) => {
        #[doc = concat!("An implementation of [`InnerListCollector`] specifically for [`NbtTag::", stringify!($array_type), "`]s.")]
        struct $name {
            list: Vec<$ty>
        }

        impl InnerListCollector for $name {
            fn accept(mut self, tag: NbtTag) -> ListCollector
            where
                Self: Sized
            {
                if let NbtTag::$single_type(v) = tag {
                    self.list.push(v);
                    ListCollector::$single_type(self)
                } else {
                    <Self as Into<InnerGenericListCollector>>::into(self)
                        .accept(tag)
                }
            }

            fn result(self) -> NbtTag {
                NbtTag::$array_type(self.list)
            }
        }

        impl $name {
            const fn new(list: Vec<$ty>) -> Self {
                Self {
                    list
                }
            }
        }

        impl From<$name> for InnerGenericListCollector {
            fn from(value: $name) -> Self {
                InnerGenericListCollector {
                    result: NbtTag::List(
                        value.list.into_iter().map(|b| NbtTag::$single_type(b)).collect()
                    )
                }
            }
        }
    };
}

add_inner_specific_array_collector_impl!(InnerIntListCollector, Int, IntArray, i32);
add_inner_specific_array_collector_impl!(InnerLongListCollector, Long, LongArray, i64);

#[cfg(test)]
mod test {
    use crate::nbt_ops::ListCollector;
    use crate::tag::NbtTag;

    #[test]
    fn list_collecting() {
        // 整数列表收集器
        let tag = NbtTag::IntArray(vec![10, 15, 20]);

        assert_eq!(
            ListCollector::new(tag).expect("列表收集器应存在").result(),
            NbtTag::IntArray(vec![10, 15, 20])
        );

        // 字节列表收集器
        let tag = NbtTag::ByteArray(vec![-1, 45, 100].into());

        assert_eq!(
            ListCollector::new(tag).expect("列表收集器应存在").result(),
            NbtTag::ByteArray(vec![-1, 45, 100].into())
        );

        // 长列表
        let tag = NbtTag::LongArray(vec![11_234_567_890, -986, 1, -937_238_122]);

        assert_eq!(
            ListCollector::new(tag).expect("列表收集器应存在").result(),
            NbtTag::LongArray(vec![11_234_567_890, -986, 1, -937_238_122])
        );

        // 通用列表收集器
        // 同构元素
        let mut collector = ListCollector::new_collector();

        collector = collector.accept(NbtTag::Float(-123.4));
        collector = collector.accept(NbtTag::Float(12.5));

        assert_eq!(
            collector.result(),
            NbtTag::List(vec![NbtTag::Float(-123.4), NbtTag::Float(12.5)])
        );

        // 异构元素
        let mut collector = ListCollector::new_collector();

        collector = collector.accept(NbtTag::Byte(99));
        collector = collector.accept(NbtTag::String("99".into()));
        collector = collector.accept(NbtTag::LongArray(vec![1, 2, 3]));

        assert_eq!(
            collector.result(),
            NbtTag::List(vec![
                NbtTag::Byte(99),
                NbtTag::String("99".into()),
                NbtTag::LongArray(vec![1, 2, 3])
            ])
        );
    }
}
