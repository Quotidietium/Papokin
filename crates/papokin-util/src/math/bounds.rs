/// 表示某个类型 `T` 的单个范围边界，该边界可以是可选的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds<T: Clone + PartialOrd> {
    min: Option<T>,
    max: Option<T>,
}

impl<T: Clone + PartialOrd> Bounds<T> {
    /// 构造一个新的 [`Bounds`]。
    pub const fn new<U: Clone + PartialOrd>(min: Option<U>, max: Option<U>) -> Bounds<U> {
        Bounds { min, max }
    }

    /// 返回此 [`Bounds`] 中的最小值与最大值是否互换了位置。
    pub fn are_swapped(&self) -> bool {
        if let Some(min) = self.min.clone()
            && let Some(max) = self.max.clone()
        {
            min > max
        } else {
            false
        }
    }
}

/// 表示一个整数范围。
/// 此范围既存储范围的边界，也存储边界的平方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct IntBounds {
    bounds: Bounds<i32>,
    squared_bounds: Bounds<i64>,
}

macro_rules! impl_square_cached_bounds {
    ($ty:ty, $normal_ty:ty, $squared_ty:ty) => {
        ///返回一对边界，其最小值和最大值为给定值。
        pub fn new(min: $normal_ty, max: $normal_ty) -> Self {
            Self::from_bounds(Bounds::<$normal_ty>::new(Some(min), Some(max)))
        }

        ///返回一对边界，其最小值为给定值。
        pub fn new_at_least(min: $normal_ty) -> Self {
            Self::from_bounds(Bounds::<$normal_ty>::new(Some(min), None))
        }

        ///返回一对边界，其最大值为给定值。
        pub fn new_at_most(max: $normal_ty) -> Self {
            Self::from_bounds(Bounds::<$normal_ty>::new(None, Some(max)))
        }

        /// 返回一个数是否满足这些边界。
        #[must_use]
        pub fn matches(&self, number: $normal_ty) -> bool {
            self.bounds.min.is_none_or(|min| min <= number)
                && self.bounds.max.is_none_or(|max| max >= number)
        }

        /// 返回一个数是否满足这些边界的平方形式。
        #[must_use]
        pub fn matches_square(&self, number: $squared_ty) -> bool {
            self.squared_bounds.min.is_none_or(|min| min <= number)
                && self.squared_bounds.max.is_none_or(|max| max >= number)
        }

        #[doc = concat!("Returns the maximum bound of this [`", stringify!($ty), "`].")]
        #[must_use]
        pub const fn min(&self) -> Option<$normal_ty> {
            self.bounds.min
        }

        /// 返回此 [`IntBounds`] 的上界。
        #[must_use]
        pub const fn max(&self) -> Option<$normal_ty> {
            self.bounds.max
        }
    };
}

impl IntBounds {
    /// 使用提供的 [`Bounds`] 创建新的 [`IntBounds`]。
    pub fn from_bounds(bounds: Bounds<i32>) -> Self {
        Self {
            bounds,
            squared_bounds: Bounds {
                min: bounds.min.map(|m| (m as i64) * (m as i64)),
                max: bounds.max.map(|m| (m as i64) * (m as i64)),
            },
        }
    }

    impl_square_cached_bounds!(IntBounds, i32, i64);
}

/// 表示一个 `f64` 范围。
/// 此范围既存储范围的边界，也存储边界的平方。
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct DoubleBounds {
    bounds: Bounds<f64>,
    squared_bounds: Bounds<f64>,
}

impl DoubleBounds {
    /// 使用提供的 [`Bounds`] 创建新的 [`DoubleBounds`]。
    pub fn from_bounds(bounds: Bounds<f64>) -> Self {
        Self {
            bounds,
            squared_bounds: Bounds {
                min: bounds.min.map(|m| m * m),
                max: bounds.max.map(|m| m * m),
            },
        }
    }

    impl_square_cached_bounds!(DoubleBounds, f64, f64);
}

/// 表示一个角度范围，以 `f32` 存储。
/// 此范围仅存储最小和最大角度值。
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct FloatDegreeBounds {
    bounds: Bounds<f32>,
}

impl FloatDegreeBounds {
    /// 使用提供的 [`Bounds`] 创建新的 [`FloatDegreeBounds`]。
    pub const fn from_bounds(bounds: Bounds<f32>) -> Self {
        Self { bounds }
    }

    /// 返回此 [`FloatDegreeBounds`] 的最小角度值。
    #[must_use]
    pub const fn min(&self) -> Option<f32> {
        self.bounds.min
    }

    /// 返回此 [`FloatDegreeBounds`] 的最大角度值。
    #[must_use]
    pub const fn max(&self) -> Option<f32> {
        self.bounds.max
    }
}
