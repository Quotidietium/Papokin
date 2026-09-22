use std::fmt::{Debug, Display};

use crate::{Number, data_result::DataResult, dynamic_ops::DynamicOps, map_like::MapLike};

use crate::lifecycle::Lifecycle;
use crate::struct_builder::{ResultStructBuilder, StringStructBuilder, StructBuilder};
use crate::{impl_string_struct_builder, impl_struct_builder};
use serde_json::{Map, Value};
use tracing::warn;

/// 一个用于向 JSON 数据序列化/从 JSON 数据反序列化的 [`DynamicOps`]。
pub struct JsonOps;

impl JsonOps {
    /// 一个将 JSON 值取为字符串的函数，类似于 Google GSON 中 `JsonElement` 的 `getAsString()` 方法。
    /// 这是为了与检查 `compressed` 的 `JsonOps` 方法保持一致。
    ///
    /// 特别地，此方法*仅*可能对以下项返回 `Some`：
    /// - 布尔值（始终）
    /// - 数字（始终）
    /// - 字符串（始终）
    /// - 恰好包含 1 个元素的数组（此时会针对该元素调用）。
    ///
    /// 其他情况返回 `None`。
    fn get_as_string(input: &Value) -> Option<String> {
        match input {
            Value::Array(elements) => {
                // 如果我们有数组，它必须只有 1 个元素。
                if elements.len() == 1 {
                    Self::get_as_string(&elements[0])
                } else {
                    None
                }
            }
            Value::Bool(b) => Some(b.to_string()),
            Value::Number(n) => Some(n.to_string()),
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// JSON 值是否被视为有效的键。
    ///
    /// 如果此方法返回 `true`，可以断定：用 `input` 调用 [`get_as_string`] 必定返回 [`Some`]。
    const fn is_valid_key(input: &Value) -> bool {
        matches!(input, Value::String(_))
    }
}

impl DynamicOps for JsonOps {
    type Value = Value;
    type StructBuilder = JsonStructBuilder;

    fn empty(&self) -> Self::Value {
        Value::Null
    }

    fn create_number(&self, n: Number) -> Self::Value {
        n.into()
    }

    fn create_bool(&self, data: bool) -> Self::Value {
        Value::Bool(data)
    }

    fn create_string(&self, data: &str) -> Self::Value {
        Value::String(data.to_owned())
    }

    fn create_list<I>(&self, values: I) -> Self::Value
    where
        I: IntoIterator<Item = Self::Value>,
    {
        Value::Array(values.into_iter().collect())
    }

    fn create_map<I>(&self, entries: I) -> Self::Value
    where
        I: IntoIterator<Item = (Self::Value, Self::Value)>,
    {
        Value::Object(
            entries
                .into_iter()
                .filter_map(|(k, v)| Self::get_as_string(&k).map(|k| (k, v)))
                .collect(),
        )
    }

    fn get_bool(&self, input: &Self::Value) -> DataResult<bool> {
        if let Value::Bool(b) = input {
            DataResult::new_success(*b)
        } else {
            DataResult::new_error(format!("Not a boolean: {input}"))
        }
    }

    fn get_number(&self, input: &Self::Value) -> DataResult<Number> {
        if let Value::Number(_) = input {
            input.try_into().map_or_else(
                |_| DataResult::new_error(format!("Not a number: {input}")),
                DataResult::new_success,
            )
        } else {
            DataResult::new_error(format!("Not a number: {input}"))
        }
    }

    fn get_string(&self, input: &Self::Value) -> DataResult<String> {
        if let Value::String(s) = input {
            DataResult::new_success(s.clone())
        } else {
            DataResult::new_error(format!("Not a string: {input}"))
        }
    }

    fn get_map_iter<'a>(
        &self,
        input: &'a Self::Value,
    ) -> DataResult<impl Iterator<Item = (Self::Value, &'a Self::Value)> + 'a> {
        if let Value::Object(map) = input {
            DataResult::new_success(map.iter().map(|(k, v)| (Value::String(k.clone()), v)))
        } else {
            DataResult::new_error(format!("Not a JSON object: {input}"))
        }
    }

    fn get_map<'a>(
        &self,
        input: &'a Self::Value,
    ) -> DataResult<impl MapLike<Value = Self::Value> + 'a> {
        if let Value::Object(map) = input {
            DataResult::new_success(JsonMapLike { map })
        } else {
            DataResult::new_error(format!("Not a JSON object: {input}"))
        }
    }

    fn get_iter(&self, input: Self::Value) -> DataResult<impl Iterator<Item = Self::Value>> {
        // 这只适用于 JSON 数组。
        if let Value::Array(list) = input {
            DataResult::new_success(list.into_iter())
        } else {
            DataResult::new_error(format!("Not a JSON array: {input}"))
        }
    }

    fn merge_into_list(&self, list: Self::Value, value: Self::Value) -> DataResult<Self::Value> {
        if matches!(list, Value::Array(_)) || list == self.empty() {
            let mut result_vec = vec![];
            if let Value::Array(a) = list {
                result_vec.extend(a);
            }

            result_vec.push(value);

            DataResult::new_success(Value::Array(result_vec))
        } else {
            DataResult::new_partial_error(format!("Not a list: {list}"), list)
        }
    }

    fn merge_values_into_list<I>(&self, list: Self::Value, values: I) -> DataResult<Self::Value>
    where
        I: IntoIterator<Item = Self::Value>,
    {
        if matches!(list, Value::Array(_)) || list == self.empty() {
            let mut result_vec = vec![];
            if let Value::Array(a) = list {
                result_vec.extend(a);
            }

            result_vec.extend(values);

            DataResult::new_success(Value::Array(result_vec))
        } else {
            DataResult::new_partial_error(format!("Not a list: {list}"), list)
        }
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
        if !matches!(map, Value::Object(_)) && map != self.empty() {
            return DataResult::new_partial_error(format!("Not a map: {map}"), map);
        }

        if !Self::is_valid_key(&key) {
            return DataResult::new_partial_error(format!("Key is not a string: {key}"), map);
        }
        let mut output_map = Map::new();

        if let Value::Object(mut m) = map {
            output_map.append(&mut m);
        }
        if let Some(key_str) = Self::get_as_string(&key) {
            output_map.insert(key_str, value);
        }

        DataResult::new_success(Value::Object(output_map))
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
        if matches!(map, Value::Object(_)) || map == self.empty() {
            let mut output_map = Map::new();

            if let Value::Object(mut m) = map {
                output_map.append(&mut m);
            }

            // 保存遗漏的条目。
            let mut missed = vec![];

            for entry in other_map_like.iter() {
                if let Some(key_str) = Self::get_as_string(&entry.0) {
                    output_map.insert(key_str, entry.1.clone());
                } else {
                    missed.push(entry.0);
                }
            }

            let object = Value::Object(output_map);
            if missed.is_empty() {
                DataResult::new_success(object)
            } else {
                DataResult::new_partial_error(
                    format!("Some keys are not valid: {missed:?}"),
                    object,
                )
            }
        } else {
            DataResult::new_partial_error(format!("Not a map: {map}"), map)
        }
    }

    fn remove(&self, input: Self::Value, key: &str) -> Value {
        if let Value::Object(m) = input {
            Value::Object(m.into_iter().filter(|(k, _)| k != key).collect())
        } else {
            input
        }
    }

    fn convert_to<U>(&self, out_ops: &impl DynamicOps<Value = U>, input: Self::Value) -> U {
        match input {
            Value::Null => out_ops.empty(),
            Value::Bool(b) => out_ops.create_bool(b),
            Value::String(s) => out_ops.create_string(&s),
            Value::Array(_) => self.convert_list(out_ops, input),
            Value::Object(_) => self.convert_map(out_ops, input),

            Value::Number(n) => {
                // 首先，检查可能的整数
                if let Some(l) = n.as_i64() {
                    if (l as i8) as i64 == l {
                        return out_ops.create_byte(l as i8);
                    } else if (l as i16) as i64 == l {
                        return out_ops.create_short(l as i16);
                    } else if (l as i32) as i64 == l {
                        return out_ops.create_int(l as i32);
                    }
                    out_ops.create_long(l)
                // 如果不可能为整数，则检查可能的浮点值。
                } else if let Some(f) = n.as_f64() {
                    if (f as f32) as f64 == f {
                        return out_ops.create_float(f as f32);
                    }
                    out_ops.create_double(f)
                } else {
                    // 以防万一。
                    warn!("转换时无法将数字放入 JSON：{n}");
                    out_ops.create_double(0.0)
                }
            }
        }
    }

    fn map_builder(&'static self) -> Self::StructBuilder {
        JsonStructBuilder {
            builder: DataResult::new_success_with_lifecycle(
                Value::Object(Map::new()),
                Lifecycle::Stable,
            ),
        }
    }
}

/// 针对 JSON 对象的 [`MapLike`] 实现。
/// 生命周期与被引用的映射相同。
struct JsonMapLike<'a> {
    map: &'a Map<String, Value>,
}

impl MapLike for JsonMapLike<'_> {
    type Value = Value;

    fn get(&self, key: &Self::Value) -> Option<&Self::Value> {
        JsonOps::get_as_string(key).and_then(|s| self.get_str(&s))
    }

    fn get_str(&self, key: &str) -> Option<&Self::Value> {
        self.map.get(key)
    }

    fn iter(&self) -> impl Iterator<Item = (Self::Value, &Self::Value)> + '_ {
        self.map.iter().map(|(k, v)| (Value::String(k.clone()), v))
    }
}

impl Display for JsonMapLike<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.map.fmt(f)
    }
}

/// 针对 JSON 对象的 [`StructBuilder`] 实现。
pub struct JsonStructBuilder {
    builder: DataResult<Value>,
}

impl ResultStructBuilder for JsonStructBuilder {
    type Result = Value;

    fn build_with_builder(
        self,
        builder: Self::Result,
        prefix: Self::Value,
    ) -> DataResult<Self::Value> {
        match prefix {
            Value::Null => DataResult::new_success(builder),
            Value::Object(mut map) => {
                match builder {
                    Value::Object(builder_map) => {
                        for (k, v) in builder_map {
                            map.insert(k, v);
                        }
                    }
                    // 这本不该发生，但以防万一。
                    _ => {
                        return DataResult::new_error(format!(
                            "Expected object in builder, found {builder}"
                        ));
                    }
                }
                DataResult::new_success(Value::Object(map))
            }
            _ => DataResult::new_partial_error(format!("Prefix is not a map: {prefix}"), prefix),
        }
    }
}

impl StructBuilder for JsonStructBuilder {
    type Value = Value;

    impl_struct_builder!(builder);
    impl_string_struct_builder!(builder, JsonOps);
}

impl StringStructBuilder for JsonStructBuilder {
    fn append(&self, key: &str, value: Self::Value, mut builder: Self::Result) -> Self::Result {
        if let Some(obj) = builder.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        builder
    }
}
