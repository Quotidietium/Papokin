use crate::lifecycle::Lifecycle;

/// 若 `DataResult` 为错误，则收集其中的部分值与消息。
///返回所提供 `DataResult` 的 [`Option`]。
/// - 部分值被存储到 `$partial_name` 中。
/// - 若找到消息，则将其推入 `$messages_vec`。
macro_rules! collect_partial_and_message {
    ($partial_name:ident, $result:ident, $messages_vec:ident) => {
        let $partial_name = match $result {
            DataResult::Success { result, .. } => Some(result),
            DataResult::Error {
                message,
                partial_result,
                ..
            } => {
                $messages_vec.push(message);
                partial_result
            }
        };
    };
}

/// 一个宏，用于生成一个函数，将某个函数应用到 `n` 个 `DataResult` 的每个结果上。
macro_rules! impl_apply {
    (@internal_method $self:ident $f:ident $($result:ident),+) => {
        let result_1 = $self;
        if !(result_1.is_error() $(|| $result.is_error())+) {
            // 全部 n 个结果均成功。
            #[allow(clippy::expect_used)]
            return DataResult::new_success($f(
                result_1.into_result().expect("已检查 is_error")
                $( , $result.into_result().expect("已检查 is_error") )+
            ));
        }
        let mut messages: Vec<String> = vec![];

        // 收集发现的任何错误。
        collect_partial_and_message!(partial_1, result_1, messages);
        $( collect_partial_and_message!($result, $result, messages); )+

        return DataResult::new_option_error_with_lifecycle(
            messages.join("; "),
            match (partial_1, $($result, )+) {
                (Some(result_1), $(Some($result), )+) => Some($f(result_1 $(, $result )+)),
                _ => None,
            },
            Lifecycle::Experimental,
        );
    };

    ($name:ident, $n:literal, $($ty:ident, $result:ident),+) => {
        #[doc = concat!("Applies a function to each result of ", stringify!($n), " `DataResult`s of different types.")]
        ///
        /// - 若给定的结果中任意一个是非结果，则返回的结果也将是非结果。
        /// - 在错误结果（非结果或部分结果）中发现的任何错误都会被添加到返回的结果中。
        /// - 若所有结果至少都是部分结果，则会调用 `f`，它应返回最终条目，该条目将被包装在返回的结果中。
        ///
        /// **当且仅当**所有给定的结果都成功时，返回的结果才是*成功*。
        pub fn $name<$($ty,)+ T>(
            self,
            f: impl FnOnce(R $(,$ty)+) -> T
            $(, $result: DataResult<$ty>)+
        ) -> DataResult<T> {
            impl_apply!(@internal_method self f $($result),+);
        }
    };
    (expect $name:ident, $n:literal, $($ty:ident, $result:ident),+) => {
        #[doc = concat!("Applies a function to each result of ", stringify!($n), " `DataResult`s of different types.")]
        ///
        /// - 若给定的结果中任意一个是非结果，则返回的结果也将是非结果。
        /// - 在错误结果（非结果或部分结果）中发现的任何错误都会被添加到返回的结果中。
        /// - 若所有结果至少都是部分结果，则会调用 `f`，它应返回最终条目，该条目将被包装在返回的结果中。
        ///
        /// **当且仅当**所有给定的结果都成功时，返回的结果才是*成功*。
        #[expect(clippy::too_many_arguments)]
        pub fn $name<$($ty,)+ T>(
            self,
            f: impl FnOnce(R $(,$ty)+) -> T
            $(, $result: DataResult<$ty>)+
        ) -> DataResult<T> {
            impl_apply!(@internal_method self f $($result),+);
        }
    };
}

/// 一个结果，既可以表示成功的结果，也可以表示
/// 带有错误的 *partial* 或非结果。
///
/// `R` 是所存储结果的类型。
#[derive(Clone, Debug)]
#[must_use]
pub enum DataResult<R> {
    /// 包含完整结果且没有错误。
    Success { result: R, lifecycle: Lifecycle },
    /// 包含空结果或部分结果，且存在错误。
    /// 该错误是一个 *格式化字符串*。
    Error {
        partial_result: Option<R>,
        lifecycle: Lifecycle,
        message: String,
    },
}

impl<R> DataResult<R> {
    /// 返回此 `DataResult` 的生命周期。
    pub const fn lifecycle(&self) -> Lifecycle {
        match self {
            Self::Success { lifecycle, .. } | Self::Error { lifecycle, .. } => *lifecycle,
        }
    }

    /// 设置此 `DataResult` 的生命周期并返回新的结果。
    pub fn with_lifecycle(self, new_lifecycle: Lifecycle) -> Self {
        match self {
            Self::Success { result, .. } => Self::Success {
                result,
                lifecycle: new_lifecycle,
            },
            Self::Error {
                partial_result,
                message,
                ..
            } => Self::Error {
                partial_result,
                message,
                lifecycle: new_lifecycle,
            },
        }
    }

    /// 将另一个 `Lifecycle` 合并到此 `DataResult` 的生命周期并返回新结果。
    pub fn add_lifecycle(self, added_lifecycle: Lifecycle) -> Self {
        let new_lifecycle = self.lifecycle().add(added_lifecycle);
        self.with_lifecycle(new_lifecycle)
    }

    ///返回一个带有实验性生命周期的*成功* `DataResult`。
    #[inline]
    pub const fn new_success(result: R) -> Self {
        Self::new_success_with_lifecycle(result, Lifecycle::Experimental)
    }

    ///返回一个带有给定生命周期的*成功* `DataResult`。
    #[inline]
    pub const fn new_success_with_lifecycle(result: R, lifecycle: Lifecycle) -> Self {
        Self::Success { result, lifecycle }
    }

    ///返回一个不带结果且带有实验性生命周期的*出错* `DataResult`。
    #[inline]
    pub fn new_error(error: impl Into<String>) -> Self {
        Self::new_error_with_lifecycle(error.into(), Lifecycle::Experimental)
    }

    ///返回一个带有部分结果和实验性生命周期的*出错* `DataResult`。
    #[inline]
    pub fn new_partial_error(error: impl Into<String>, partial_result: R) -> Self {
        Self::new_partial_error_with_lifecycle(
            error.into(),
            partial_result,
            Lifecycle::Experimental,
        )
    }

    ///返回一个不带结果且带有给定生命周期的*出错* `DataResult`。
    #[inline]
    pub fn new_error_with_lifecycle<T>(
        message: impl Into<String>,
        lifecycle: Lifecycle,
    ) -> DataResult<T> {
        DataResult::Error {
            partial_result: None,
            lifecycle,
            message: message.into(),
        }
    }

    ///返回一个带有部分结果和给定生命周期的*出错* `DataResult`。
    #[inline]
    pub fn new_partial_error_with_lifecycle(
        message: impl Into<String>,
        partial_result: R,
        lifecycle: Lifecycle,
    ) -> Self {
        Self::Error {
            partial_result: Some(partial_result),
            lifecycle,
            message: message.into(),
        }
    }

    ///返回一个带有结果 [`Option<R>`] 和给定生命周期的*出错* `DataResult`。
    #[inline]
    const fn new_option_error_with_lifecycle(
        message: String,
        partial_result: Option<R>,
        lifecycle: Lifecycle,
    ) -> Self {
        Self::Error {
            partial_result,
            lifecycle,
            message,
        }
    }

    /// 尝试从此 `DataResult` 获取完整结果。如果不存在这样的结果，则返回 [`None`]（即使是部分结果也是如此）。
    ///
    /// 若要允许部分结果，请使用 [`DataResult::into_result_or_partial`]。
    #[inline]
    pub fn into_result(self) -> Option<R> {
        if let Self::Success { result, .. } = self {
            Some(result)
        } else {
            None
        }
    }

    /// 尝试获取完整或部分结果。如果不存在这样的结果，则返回 [`None`]。
    pub fn into_result_or_partial(self) -> Option<R> {
        match self {
            Self::Success { result, .. } => Some(result),
            Self::Error { partial_result, .. } => partial_result,
        }
    }

    /// 尝试以引用形式获取完整或部分结果。如果不存在这样的结果，则返回 [`None`]。
    pub const fn result_or_partial_as_ref(&self) -> Option<&R> {
        match self {
            Self::Success { result, .. } => Some(result),
            Self::Error { partial_result, .. } => partial_result.as_ref(),
        }
    }

    /// 尝试从此 `DataResult` 获取完整结果。如果不存在这样的结果，此函数会 panic。
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub fn unwrap(self) -> R {
        self.expect("DataResult 中没有完整结果")
    }

    /// 尝试从此 `DataResult` 获取完整或部分结果。如果不存在这样的结果，此函数会 panic。
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub fn unwrap_or_partial(self) -> R {
        self.expect_or_partial("DataResult 中没有完整或部分结果")
    }

    /// 尝试从此 `DataResult` 获取完整结果。如果不存在这样的结果，此函数会以自定义消息 panic。
    #[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    pub fn expect(self, message: &str) -> R {
        self.into_result().unwrap_or_else(|| panic!("{}", message))
    }

    /// 尝试从此 `DataResult` 获取完整或部分结果。如果不存在这样的结果，此函数会以自定义消息 panic。
    #[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    pub fn expect_or_partial(self, message: &str) -> R {
        self.into_result_or_partial()
            .unwrap_or_else(|| panic!("{}", message))
    }

    /// 返回此 `DataResult` 是否拥有完整或部分结果。
    pub const fn has_result_or_partial(&self) -> bool {
        !matches!(
            self,
            Self::Error {
                partial_result: None,
                ..
            }
        )
    }

    /// 将两个消息拼接成一个更大的消息。
    /// 这对于为包含 1 个以上错误的数据结果堆叠消息很有用。
    #[must_use]
    pub fn append_messages(first: &str, second: &str) -> String {
        format!("{first}; {second}")
    }

    /// 通过应用函数将类型 `R` 的 `DataResult` 映射为类型 `T` 的 `DataResult`，非结果值保持不变。
    ///
    /// `f` 会应用于完整结果与部分结果。对部分结果而言，`f` 应用于其部分值。
    pub fn map<T>(self, op: impl FnOnce(R) -> T) -> DataResult<T> {
        match self {
            Self::Success { result, lifecycle } => {
                DataResult::new_success_with_lifecycle(op(result), lifecycle)
            }
            Self::Error {
                partial_result,
                lifecycle,
                message,
            } => DataResult::new_option_error_with_lifecycle(
                message,
                partial_result.map(op),
                lifecycle,
            ),
        }
    }

    /// 将类型 `R` 的 `DataResult` 映射为类型 `T`。
    /// - 若存在完整结果，则以该结果调用 `f`（结果函数）。
    /// - 否则，如果存在错误，则以该错误为参数调用 `default`（错误函数）。
    pub fn map_or_else<T>(self, default: impl FnOnce(Self) -> T, f: impl Fn(R) -> T) -> T {
        match self {
            Self::Success { result, .. } => f(result),
            Self::Error { .. } => default(self),
        }
    }

    /// 将一个 `DataResult` 与另一个接受 `DataResult` 的函数串联起来。
    /// - 若存在完整或部分结果，则以该结果调用 `f`，并返回 `f` 返回的值。
    ///   对于部分结果，新消息通过拼接进行传播。
    /// - 否则，如果存在无结果的错误，则传播该错误 `DataResult`。
    ///
    /// 也就是说，`f` 将处理此 `DataResult` 的完整或部分结果（若有），并在必要时附加错误。
    ///
    /// 此函数的名称等价于 `and_then`。
    pub fn flat_map<T>(self, f: impl FnOnce(R) -> DataResult<T>) -> DataResult<T> {
        match self {
            Self::Success { result, lifecycle } => {
                // 将此 DataResult 的生命周期添加到新的 DataResult 中。
                f(result).add_lifecycle(lifecycle)
            }
            Self::Error {
                partial_result,
                lifecycle,
                message,
            } => {
                if let Some(result) = partial_result {
                    // 尝试映射内部的部分值。
                    let second_result = f(result);
                    let new_lifecycle = second_result.lifecycle().add(lifecycle);
                    match second_result {
                        DataResult::Success { result, .. } => {
                            DataResult::new_partial_error_with_lifecycle(
                                message,
                                result,
                                new_lifecycle,
                            )
                        }
                        DataResult::Error {
                            partial_result,
                            message: second_message,
                            ..
                        } => DataResult::new_option_error_with_lifecycle(
                            Self::append_messages(&message, &second_message),
                            partial_result,
                            new_lifecycle,
                        ),
                    }
                } else {
                    // 返回同一个 Error。
                    DataResult::Error {
                        partial_result: None,
                        lifecycle,
                        message,
                    }
                }
            }
        }
    }

    /// 将包裹在 `DataResult` 中的函数应用到本 `DataResult` 包裹的值上。
    pub fn apply<T>(self, function_result: DataResult<impl FnOnce(R) -> T>) -> DataResult<T> {
        let lifecycle = self.lifecycle().add(function_result.lifecycle());
        match (self, function_result) {
            (Self::Success { result, .. }, DataResult::Success { result: f, .. }) => {
                DataResult::new_success_with_lifecycle(f(result), lifecycle)
            }
            (
                Self::Success { result, .. },
                DataResult::Error {
                    partial_result,
                    message: func_message,
                    ..
                },
            ) => DataResult::new_option_error_with_lifecycle(
                func_message,
                partial_result.map(|f| f(result)),
                lifecycle,
            ),
            (
                Self::Error {
                    partial_result,
                    message,
                    ..
                },
                DataResult::Success { result: f, .. },
            ) => DataResult::new_option_error_with_lifecycle(
                message,
                partial_result.map(f),
                lifecycle,
            ),
            (
                Self::Error {
                    partial_result,
                    message,
                    ..
                },
                DataResult::Error {
                    partial_result: partial_func_result,
                    message: func_message,
                    ..
                },
            ) => DataResult::new_option_error_with_lifecycle(
                Self::append_messages(&message, &func_message),
                partial_result.and_then(|r| partial_func_result.map(|f| f(r))),
                lifecycle,
            ),
        }
    }

    /// 与 [`Self::apply_2`] 类似，但还会将返回的 `DataResult` 标记为 [`Lifecycle::Stable`]。
    pub fn apply_2_and_make_stable<R2, T>(
        self,
        f: impl FnOnce(R, R2) -> T,
        second_result: DataResult<R2>,
    ) -> DataResult<T> {
        self.apply_2(f, second_result)
            .with_lifecycle(Lifecycle::Stable)
    }

    impl_apply!(apply_2, 2, R2, second_result);
    impl_apply!(apply_3, 3, R2, result_2, R3, result_3);
    impl_apply!(apply_4, 4, R2, result_2, R3, result_3, R4, result_4);
    impl_apply!(
        apply_5, 5, R2, result_2, R3, result_3, R4, result_4, R5, result_5
    );
    impl_apply!(
        apply_6, 6, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6
    );
    impl_apply!(expect apply_7, 7, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7);
    impl_apply!(expect apply_8, 8, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8);
    impl_apply!(expect apply_9, 9, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9);
    impl_apply!(expect apply_10, 10, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10);
    impl_apply!(expect apply_11, 11, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10, R11, result_11);
    impl_apply!(expect apply_12, 12, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10, R11, result_11, R12, result_12);
    impl_apply!(expect apply_13, 13, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10, R11, result_11, R12, result_12, R13, result_13);
    impl_apply!(expect apply_14, 14, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10, R11, result_11, R12, result_12, R13, result_13, R14, result_14);
    impl_apply!(expect apply_15, 15, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10, R11, result_11, R12, result_12, R13, result_13, R14, result_14, R15, result_15);
    impl_apply!(expect apply_16, 16, R2, result_2, R3, result_3, R4, result_4, R5, result_5, R6, result_6, R7, result_7, R8, result_8, R9, result_9, R10, result_10, R11, result_11, R12, result_12, R13, result_13, R14, result_14, R15, result_15, R16, result_16);

    /// 对 `DataResult` 的错误应用函数，成功值保持不变。
    /// 这可用于为错误提供额外的上下文。
    pub fn map_error(self, f: impl FnOnce(String) -> String) -> Self {
        match self {
            Self::Success { .. } => self,
            Self::Error {
                message,
                lifecycle,
                partial_result,
            } => Self::new_option_error_with_lifecycle(f(message), partial_result, lifecycle),
        }
    }

    /// 将包含部分结果的 `DataResult` 提升为成功的 `DataResult`，并提供
    /// 错误消息传给函数 `f`（消费者函数），并将其从新的 `DataResult` 中移除。
    /// 没有结果或已有完整结果的 `DataResult` 保持不变。
    pub fn promote_partial(self, f: impl FnOnce(String)) -> Self {
        match self {
            Self::Success { .. } => self,
            Self::Error {
                message,
                lifecycle,
                partial_result,
            } => {
                f(message.clone());
                partial_result.map_or_else(
                    || Self::new_error_with_lifecycle(message, lifecycle),
                    |result| Self::new_success_with_lifecycle(result, lifecycle),
                )
            }
        }
    }

    ///返回一个带有新部分值（始终为部分值）的 `DataResult`，不改动已含完整结果的 `DataResult`。
    pub fn with_partial(self, partial_value: R) -> Self {
        match self {
            Self::Success { .. } => self,
            Self::Error {
                message, lifecycle, ..
            } => Self::new_partial_error_with_lifecycle(message, partial_value, lifecycle),
        }
    }

    ///根据此 `DataResult` 的类型，返回一个带有新结果/部分结果的 `DataResult`。
    /// - 对于完整结果，此方法返回另一个 `DataResult`，其完整结果为 `value`。
    /// - 对于部分结果，此方法返回另一个 `DataResult`，其部分结果为 `value`。
    /// - 对于非结果，此方法返回其自身。
    pub fn with_complete_or_partial<T>(self, value: T) -> DataResult<T> {
        match self {
            Self::Success { lifecycle, .. } => {
                DataResult::new_success_with_lifecycle(value, lifecycle)
            }
            Self::Error {
                message,
                lifecycle,
                partial_result: Some(_),
            } => DataResult::new_partial_error_with_lifecycle(message, value, lifecycle),
            Self::Error {
                message, lifecycle, ..
            } => Self::new_error_with_lifecycle(message, lifecycle),
        }
    }

    /// 返回此 `DataResult` 是否为一次成功。
    pub const fn is_success(&self) -> bool {
        matches!(self, &Self::Success { .. })
    }

    /// 返回此 `DataResult` 是否为错误（包括部分结果错误）。
    pub const fn is_error(&self) -> bool {
        !self.is_success()
    }

    /// 将另一个 `DataResult`（`other_result`）的消息添加到此 `DataResult`。
    ///
    /// 这对于仅用作复杂对象最终结果的*单元元组* `DataResult` 很有用。
    /// - 若 `other_result` 是完整结果，则不发生任何事。
    /// - 若两个结果均为部分结果，则返回的结果也是部分结果；否则为非结果。
    /// - 任何 `DataResult` 错误中包含的消息会被拼接并用于返回的结果。
    ///
    /// 这总是返回*稳定*的结果。
    pub fn add_message<T>(self, other_result: &DataResult<T>) -> Self {
        match (self, other_result) {
            // 两个结果都成功。
            (Self::Success { result: r, .. }, DataResult::Success { .. }) => {
                Self::new_success_with_lifecycle(r, Lifecycle::Stable)
            }

            // 两个结果都是错误。
            (
                Self::Error {
                    partial_result: p1,
                    message: m1,
                    ..
                },
                DataResult::Error {
                    partial_result: p2,
                    message: m2,
                    ..
                },
            ) => Self::new_option_error_with_lifecycle(
                Self::append_messages(&m1, m2),
                if p1.is_some() && p2.is_some() {
                    p1
                } else {
                    None
                },
                Lifecycle::Stable,
            ),

            // 两个结果中恰好有一个是错误
            (
                Self::Error {
                    message: m1,
                    partial_result,
                    ..
                },
                _,
            ) => Self::new_option_error_with_lifecycle(m1, partial_result, Lifecycle::Stable),

            (
                Self::Success { result, .. },
                DataResult::Error {
                    message: m2,
                    partial_result,
                    ..
                },
            ) => Self::new_option_error_with_lifecycle(
                m2.clone(),
                partial_result.is_some().then_some(result),
                Lifecycle::Stable,
            ),
        }
    }

    /// 尝试从给定的 `result` 添加错误并将其加入 `self`
    /// 如果 `self` 尚不是错误结果。
    ///
    /// 返回的 `DataResult` 的 [`Lifecycle`] 是两个结果的叠加。
    pub fn with_errors_from<T>(self, result: &DataResult<T>) -> Self {
        let current_lifecycle = self.lifecycle();

        match (self, result) {
            (s @ Self::Error { .. }, _) | (s, DataResult::Success { .. }) => s,

            (
                Self::Success { result: val, .. },
                DataResult::Error {
                    message,
                    lifecycle: other_lifecycle,
                    ..
                },
            ) => Self::new_partial_error_with_lifecycle(
                message.clone(),
                val,
                current_lifecycle.add(*other_lifecycle),
            ),
        }
    }

    /// 返回此 `DataResult` 的消息（如有）。只有错误结果才有消息。
    pub fn get_message(self) -> Option<String> {
        match self {
            Self::Success { .. } => None,
            Self::Error { message, .. } => Some(message),
        }
    }
}

// 断言函数

/// 断言 `$left` 这个 `DataResult` 是一个完整结果（成功），其存储的结果为 `$right`。
#[macro_export]
macro_rules! assert_success {
    ($left:expr, $right:expr $(,)?) => {{
        let result = $left;
        assert!(
            result.is_success(),
            "预期 `DataResult` 为成功结果，实际得到：{:?}",
            result
        );
        assert_eq!(
            result.into_result().expect("DataResult 为成功结果"),
            $right,
            "`DataResult` 成功了，但值不匹配"
        );
    }};
}

/// 断言对左侧表达式的编码将产生一个完整结果（成功），其存储的结果为 `$right`。
#[macro_export]
macro_rules! assert_encode_success {
    ($left:expr, $ops:expr, $right:expr $(,)?) => {{
        let result = $crate::codec::Encode::encode_start(&$left, &$ops);
        assert!(
            result.is_success(),
            "预期 `DataResult` 为成功结果，实际得到：{:?}",
            result
        );
        assert_eq!(
            result.into_result().expect("DataResult 为成功结果"),
            $right,
            "`DataResult` 成功了，但值不匹配"
        );
    }};
}

/// 断言对左侧表达式的解码将产生一个 `DataResult`，其提供的（provided）方法返回 `true`。
#[macro_export]
macro_rules! assert_decode {
    ($ty:ty, $input:expr, $ops:expr, $func:ident $(,)?) => {{
        let result = <$ty as $crate::codec::Decode>::parse($input, &$ops);
        assert!(
            result.$func(),
            concat!(
                "预期 `DataResult` 对 ",
                stringify!($func),
                " 返回 `true`，实际得到：{:?}"
            ),
            result
        );
    }};
}

impl<T> Default for DataResult<T> {
    fn default() -> Self {
        Self::new_error("Default DataResult")
    }
}

/// 一种从其他类型转换到本类型、可能失败并产生 [`DataResult`] 的类型转换。
///
/// 实现转换时，始终优先使用 [`FlatTryFrom`] 而非 [`FlatTryInto`]，
/// 作为 [`FlatTryInto`] 的实现也会自动可用。
pub trait FlatTryFrom<T>: Sized {
    /// 执行转换。
    fn flat_try_from(value: T) -> DataResult<Self>;
}

impl<T> FlatTryFrom<T> for T {
    fn flat_try_from(value: T) -> DataResult<Self> {
        DataResult::new_success(value)
    }
}

impl<T, U> FlatTryInto<U> for T
where
    U: FlatTryFrom<T>,
{
    #[inline]
    /// 调用 `U::flat_try_from()` 来执行转换。
    fn flat_try_into(self) -> DataResult<U> {
        U::flat_try_from(self)
    }
}

/// 一种从本类型转换到其他类型、可能失败并产生 [`DataResult`] 的类型转换。
///
/// 实现转换时，始终优先使用 [`FlatTryFrom`] 而非 [`FlatTryInto`]，
/// 作为 [`FlatTryInto`] 的实现也会自动可用。
pub trait FlatTryInto<T>: Sized {
    /// 执行转换。
    fn flat_try_into(self) -> DataResult<T>;
}
