use crate::data_result::DataResult;
use crate::dynamic_ops::DynamicOps;

/// 用于构建列表的 trait。
#[must_use]
pub trait ListBuilder {
    type Value;

    /// 构建最终列表并返回结果。
    fn build(self, prefix: Self::Value) -> DataResult<Self::Value>;

    /// 向此 [`ListBuilder`] 添加一个直接值。
    #[must_use]
    fn add(self, value: Self::Value) -> Self;

    /// 向此 [`ListBuilder`] 添加一个 [`DataResult`]。
    #[must_use]
    fn add_data_result(self, value: DataResult<Self::Value>) -> Self;
}

/// [`ListBuilder`] 的实现。
pub struct ListBuilderImpl<T, O: 'static> {
    elements: DataResult<Vec<T>>,
    ops: &'static O,
}

pub fn new_list_builder_impl<T>(
    ops: &'static impl DynamicOps<Value = T>,
) -> impl ListBuilder<Value = T> {
    ListBuilderImpl {
        elements: DataResult::new_success(vec![]),
        ops,
    }
}

impl<T, O> ListBuilder for ListBuilderImpl<T, O>
where
    O: DynamicOps<Value = T>,
{
    type Value = T;

    fn build(self, prefix: Self::Value) -> DataResult<Self::Value> {
        self.elements
            .flat_map(|e| self.ops.merge_values_into_list(prefix, e))
    }

    fn add(mut self, value: Self::Value) -> Self {
        self.elements = self.elements.map(|mut e: Vec<T>| {
            e.push(value);
            e
        });
        self
    }

    fn add_data_result(mut self, value: DataResult<T>) -> Self {
        self.elements = self.elements.apply_2_and_make_stable(
            |mut e, v| {
                e.push(v);
                e
            },
            value,
        );
        self
    }
}
