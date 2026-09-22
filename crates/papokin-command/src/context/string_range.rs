use std::ops::Range;

use crate::string_reader::StringReader;

/// 表示一个实际上
/// 从字符串的 `start` 处开始的子串
/// 以及 `end` 字节索引。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringRange {
    pub start: usize,
    pub end: usize,
}

impl StringRange {
    /// 构造一个新的子串范围，其索引
    /// 对 `start` 是包含的，对 `end` 则是排除的。
    #[must_use]
    pub const fn between(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// 在给定位置的左侧构造一个空范围
    /// 索引为 `pos` 的单个字符。
    #[must_use]
    pub const fn at(pos: usize) -> Self {
        Self::between(pos, pos)
    }

    /// 构造一个涵盖两个位置的新子串范围
    /// [`StringRange`]，并返回一个覆盖
    /// 两个必需的范围。
    #[must_use]
    pub fn encompass(a: Self, b: Self) -> Self {
        Self::between(a.start.min(b.start), a.end.max(b.end))
    }

    /// 获取 [`str`] 子字符串切片的边界
    /// 从此范围创建一个 [`StringReader`]。
    #[must_use]
    pub fn slice_from_reader<'a>(&self, reader: &'a StringReader) -> &'a str {
        &reader.string()[self.start..self.end]
    }

    /// 获取 [`str`] 子字符串切片的边界
    /// 从此范围创建一个 [`String`]。
    #[must_use]
    pub fn substring_slice<'a>(&self, string: &'a str) -> &'a str {
        &string[self.start..self.end]
    }

    ///返回此范围的长度是否为零。
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// 返回此范围的长度。
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }
}

impl From<Range<usize>> for StringRange {
    fn from(value: Range<usize>) -> Self {
        Self::between(value.start, value.end)
    }
}

impl From<StringRange> for Range<usize> {
    fn from(value: StringRange) -> Self {
        value.start..value.end
    }
}
