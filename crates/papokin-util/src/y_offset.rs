use serde::Deserialize;

/// 表示用于解析 Y 坐标的垂直偏移定义。
///
/// 此值可以用三种不同的方式表示：
/// - 作为绝对的世界 Y 值。
/// - 作为世界最小 Y 之上的方块数。
/// - 作为世界顶部边界之下的方块数。
///
/// 该枚举以无标签形式反序列化，也就是说变体
/// 根据提供的字段进行选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum YOffset {
    /// 绝对世界 Y 坐标。
    Absolute(Absolute),
    /// 从世界最小 Y 向上度量的偏移量。
    AboveBottom(AboveBottom),
    /// 从世界最大 Y 向下度量的偏移量。
    BelowTop(BelowTop),
}

impl YOffset {
    /// 解析生效的世界 Y 坐标。
    ///
    /// # Arguments
    /// * `min_y` - 世界的最小 Y 层级。
    /// * `height` - 世界的总高度。
    ///
    /// # Returns
    /// 计算得到的绝对 Y 坐标，类型为 `i32`。
    ///
    /// # Behaviour
    /// * `Absolute` - 直接返回给定的 Y 坐标。
    /// * `AboveBottom` - 返回 `min_y + offset`。
    /// * `BelowTop` - 返回 `(min_y + height - 1) - offset`。
    #[must_use]
    pub const fn get_y(&self, min_y: i16, height: u16) -> i32 {
        match self {
            Self::AboveBottom(above_bottom) => min_y as i32 + above_bottom.above_bottom as i32,
            Self::BelowTop(below_top) => {
                height as i32 - 1 + min_y as i32 - below_top.below_top as i32
            }
            Self::Absolute(absolute) => absolute.absolute as i32,
        }
    }
}

/// 世界坐标中的绝对垂直位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Absolute {
    /// 固定的世界 Y 高度。
    pub absolute: i16,
}

/// 从世界最小 Y 层级向上度量的偏移量。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct AboveBottom {
    /// 高于最小 Y 的方块数。
    pub above_bottom: i8,
}

/// 从世界顶部边界向下度量的偏移量。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct BelowTop {
    /// 低于最大 Y 的方块数。
    pub below_top: i8,
}
