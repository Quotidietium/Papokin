/// 一个用于传达某个对象生命周期的标记。
/// 它可能是稳定的、实验性的或已弃用的。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// 表示稳定的事物。
    Stable,
    /// 表示实验性的事物。
    Experimental,
    /// 表示已弃用的事物。带此生命周期的 `u32` 表示其被标记为弃用的*日期*。
    ///
    /// 数字更小表示更早被弃用，更大则表示更晚被弃用。
    Deprecated(u32),
}

impl Lifecycle {
    /// 将一个生命周期加到另一个上，返回与两者中更严格者一致的结果生命周期。
    ///
    /// 此函数按以下顺序执行：
    /// - 若至少有一个生命周期为 *experimental*（实验性），则返回 [`Lifecycle::Experimental`]。
    /// - 若两个生命周期均为 *deprecated*（已弃用），则返回较早弃用的那个（即 *date* 较小者）。
    /// - 若恰好只有一个生命周期为 *deprecated*（已弃用），则返回该生命周期。
    /// - 若以上均不匹配，则返回 [`Lifecycle::Stable`]。
    #[must_use]
    pub const fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Experimental, _) | (_, Self::Experimental) => Self::Experimental,

            (d1 @ Self::Deprecated(s1), d2 @ Self::Deprecated(s2)) => {
                if s1 < s2 {
                    d1
                } else {
                    d2
                }
            }

            (d @ Self::Deprecated(_), _) | (_, d @ Self::Deprecated(_)) => d,

            _ => Self::Stable,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::lifecycle::Lifecycle;

    #[test]
    fn add_lifecycles() {
        assert_eq!(Lifecycle::Stable.add(Lifecycle::Stable), Lifecycle::Stable);

        assert_eq!(
            Lifecycle::Experimental.add(Lifecycle::Deprecated(10)),
            Lifecycle::Experimental
        );

        assert_eq!(
            Lifecycle::Deprecated(10).add(Lifecycle::Deprecated(15)),
            Lifecycle::Deprecated(10)
        );
    }
}
