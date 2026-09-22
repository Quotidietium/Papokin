use crate::data_result::DataResult;
use crate::dynamic_ops::DynamicOps;
use crate::lifecycle::Lifecycle;

/// 一个指定生成器的 trait，用于添加键值对以创建复合类型。
///
/// `Value` 是此构建器的动态类型。
/// 对于结构体，此处的一些方法可以通过 `impl_struct_builder` 宏实现。
pub trait StructBuilder {
    type Value;

    /// 向此构建器添加单个键值对并返回该构建器。
    #[must_use]
    fn add_key_value(self, key: Self::Value, value: Self::Value) -> Self;

    /// 向此构建器添加单个键-“值结果”对并返回该构建器。
    #[must_use]
    fn add_key_value_result(self, key: Self::Value, value: DataResult<Self::Value>) -> Self;

    /// 向此构建器添加一对“键结果”-“值结果”并返回该构建器。
    #[must_use]
    fn add_key_result_value_result(
        self,
        key: DataResult<Self::Value>,
        value: DataResult<Self::Value>,
    ) -> Self;

    /// 将给定 `DataResult` 的错误消息（如有）添加到此构建器并返回该构建器。
    #[must_use]
    fn with_errors_from<T>(self, result: &DataResult<T>) -> Self;

    /// 向此构建器添加单个字符串键值对并返回该构建器。
    #[must_use]
    fn add_string_key_value(self, key: &str, value: Self::Value) -> Self;

    /// 向此构建器添加单个字符串键-“值结果”对并返回该构建器。
    #[must_use]
    fn add_string_key_value_result(self, key: &str, value: DataResult<Self::Value>) -> Self;

    /// 设置此构建器的生命周期并返回该构建器。
    #[must_use]
    fn set_lifecycle(self, lifecycle: Lifecycle) -> Self;

    /// 将内部构建器的错误交给函数 `f` 处理并返回该构建器。
    #[must_use]
    fn map_error(self, f: impl FnOnce(String) -> String) -> Self;

    /// 构建存储在此构建器中的映射并附带前缀，然后返回结果。
    fn build(self, prefix: Self::Value) -> DataResult<Self::Value>;

    /// 构建存储在此构建器中的映射并附带 `DataResult` 前缀，然后返回结果。
    fn build_with_result_prefix(self, prefix: DataResult<Self::Value>) -> DataResult<Self::Value>
    where
        Self: Sized,
    {
        prefix.flat_map(|p| self.build(p))
    }
}

/// 一个为构建器指定了 `Result` 类型的 [`StructBuilder`]。
pub trait ResultStructBuilder: StructBuilder {
    type Result;

    /// 构建存储在 `builder` 中的映射并附带前缀，然后返回结果。
    fn build_with_builder(
        self,
        builder: Self::Result,
        prefix: Self::Value,
    ) -> DataResult<Self::Value>;
}

/// [`StructBuilder`] 的一个子 trait，用于追加字符串键而非动态类型键。
/// `StructBuilder` 中的方法也可以通过 `impl_string_struct_builder` 宏来实现。
pub trait StringStructBuilder: ResultStructBuilder {
    /// 向 `builder` 追加一个字符串键值对，并原地修改它。
    fn append(&self, key: &str, value: Self::Value, builder: Self::Result) -> Self::Result;
}

/// [`StructBuilder`] 的一个子 trait，用于追加动态键。`StructBuilder` 中的方法
/// 也可以通过 `impl_universal_struct_builder` 宏实现。
pub trait UniversalStructBuilder: ResultStructBuilder {
    /// 向 `builder` 追加一个键值对，并原地修改它。
    fn append(&self, key: Self::Value, value: Self::Value, builder: Self::Result) -> Self::Result;
}

/// 一个宏，需放置在实现了 [`StructBuilder`] 的结构体的 `impl` 块中。
///
/// 将此放置在 `impl StructBuilder for ...` 块中。
/// 这会自动实现向构建器添加键值对的方法。
/// 请确保存在名为 `$builder`、类型为 [`DataResult<Self::Value>`] 的结构体字段。
#[macro_export]
macro_rules! impl_struct_builder {
    ($builder:ident) => {
        fn set_lifecycle(mut self, lifecycle: Lifecycle) -> Self {
            self.$builder = self.$builder.with_lifecycle(lifecycle);
            self
        }

        fn map_error(mut self, f: impl FnOnce(String) -> String) -> Self {
            self.$builder = self.$builder.map_error(f);
            self
        }

        fn with_errors_from<U>(mut self, result: &DataResult<U>) -> Self {
            self.$builder = self.$builder.with_errors_from(result);
            self
        }

        fn build(self, prefix: Self::Value) -> DataResult<Self::Value> {
            self.$builder
                .clone()
                .flat_map(|b| self.build_with_builder(b, prefix))
        }
    };
}

/// 一个宏，需放置在实现了 [`StringStructBuilder`] 的结构体的 `impl` 块中。
///
/// 将此放置在 `impl StructBuilder for ...` 块中。
/// 这会自动实现向构建器添加键值对的方法。
#[macro_export]
macro_rules! impl_string_struct_builder {
    (@internal $builder:ident) => {
        fn add_string_key_value(mut self, key: &str, value: Self::Value) -> Self {
            self.$builder = std::mem::take(&mut self.$builder).map(|r| self.append(key, value, r));
            self
        }

        fn add_string_key_value_result(mut self, key: &str, value: DataResult<Self::Value>) -> Self {
            self.$builder = std::mem::take(&mut self.$builder).apply_2_and_make_stable(|r, v| self.append(key, v, r), value);
            self
        }
    };

    // 用于常量操作
    ($builder:ident, $ops:ident) => {

        impl_string_struct_builder!(@internal $builder);

        fn add_key_value(mut self, key: Self::Value, value: Self::Value) -> Self {
            self.$builder = $ops.get_string(&key).flat_map(
                |s| std::mem::take(&mut self.$builder).map(|r| self.append(&s, value, r))
            );
            self
        }

        fn add_key_value_result(mut self, key: Self::Value, value: DataResult<Self::Value>) -> Self {
            self.$builder = $ops.get_string(&key).flat_map(
                |s| std::mem::take(&mut self.$builder).apply_2_and_make_stable(|r, v| self.append(&s, v, r), value)
            );
            self
        }

        fn add_key_result_value_result(
            mut self,
            key: DataResult<Self::Value>,
            value: DataResult<Self::Value>,
        ) -> Self {
            self.$builder = key.flat_map(|v| $ops.get_string(&v)).flat_map(|s| {
                std::mem::take(&mut self.$builder).apply_2_and_make_stable(|r, v| self.append(&s, v, r), value)
            });
            self
        }
    };

    // 针对已存储的操作
    ($builder:ident, self. $ops:ident) => {

        impl_string_struct_builder!(@internal $builder);

        fn add_key_value(mut self, key: Self::Value, value: Self::Value) -> Self {
            self.$builder = self.$ops.get_string(&key).flat_map(
                |s| std::mem::take(&mut self.$builder).map(|r| self.append(&s, value, r))
            );
            self
        }

        fn add_key_value_result(mut self, key: Self::Value, value: DataResult<Self::Value>) -> Self {
            self.$builder = self.$ops.get_string(&key).flat_map(
                |s| std::mem::take(&mut self.$builder).apply_2_and_make_stable(|r, v| self.append(&s, v, r), value)
            );
            self
        }

        fn add_key_result_value_result(
            mut self,
            key: DataResult<Self::Value>,
            value: DataResult<Self::Value>,
        ) -> Self {
            self.$builder = key.flat_map(|v| self.$ops.get_string(&v)).flat_map(|s| {
                std::mem::take(&mut self.$builder).apply_2_and_make_stable(|r, v| self.append(&s, v, r), value)
            });
            self
        }
    };
}

/// 一个宏，需放置在实现了 `UniversalStructBuilder` 的结构体的 `impl` 块中。
///
/// 将此放置在 `impl StructBuilder for ...` 块中。
/// 这会自动实现向构建器添加键值对的方法。
#[macro_export]
macro_rules! impl_universal_struct_builder {
    (@internal $builder:ident) => {
        fn add_key_value(mut self, key: Self::Value, value: Self::Value) -> Self {
            self.$builder = std::mem::take(&mut self.$builder).map(|b| self.append(key, value, b));
            self
        }

        fn add_key_value_result(mut self, key: Self::Value, value: DataResult<Self::Value>) -> Self {
            self.$builder = std::mem::take(&mut self.$builder)
                .apply_2_and_make_stable(|b, v| self.append(key, v, b), value);
            self
        }

        fn add_key_result_value_result(
            mut self,
            key: DataResult<Self::Value>,
            value: DataResult<Self::Value>,
        ) -> Self {
            self.$builder = self
                .$builder
                .clone()
                .apply(key.apply_2_and_make_stable(|k, v| (|b| self.append(k, v, b)), value));
            self
        }
    };

    // 用于常量操作
    ($builder:ident, $ops:ident) => {
        impl_universal_struct_builder!(@internal $builder);

        fn add_string_key_value(self, key: &str, value: Self::Value) -> Self {
            self.add_key_value($ops.create_string(key), value)
        }

        fn add_string_key_value_result(self, key: &str, value: DataResult<Self::Value>) -> Self {
            self.add_key_value_result($ops.create_string(key), value)
        }
    };

    // 针对已存储的操作
    ($builder:ident, self. $ops:ident) => {
        impl_universal_struct_builder!(@internal $builder);

        fn add_string_key_value(self, key: &str, value: Self::Value) -> Self {
            let string = self.$ops.create_string(key);
            self.add_key_value(string, value)
        }

        fn add_string_key_value_result(self, key: &str, value: DataResult<Self::Value>) -> Self {
            let string = self.$ops.create_string(key);
            self.add_key_value_result(string, value)
        }
    };
}

pub struct MapBuilder<T, O: DynamicOps<Value = T> + 'static> {
    builder: DataResult<Vec<(T, T)>>,
    ops: &'static O,
}

impl<T: Clone, O: DynamicOps<Value = T>> StructBuilder for MapBuilder<T, O> {
    type Value = T;

    impl_struct_builder!(builder);
    impl_universal_struct_builder!(builder, self.ops);
}

impl<T: Clone, O: DynamicOps<Value = T>> ResultStructBuilder for MapBuilder<T, O> {
    type Result = Vec<(T, T)>;

    fn build_with_builder(
        self,
        builder: Self::Result,
        prefix: Self::Value,
    ) -> DataResult<Self::Value> {
        self.ops.merge_entries_into_map(prefix, builder)
    }
}

impl<T: Clone, O: DynamicOps<Value = T>> UniversalStructBuilder for MapBuilder<T, O> {
    fn append(
        &self,
        key: Self::Value,
        value: Self::Value,
        mut builder: Self::Result,
    ) -> Self::Result {
        builder.push((key, value));
        builder
    }
}
