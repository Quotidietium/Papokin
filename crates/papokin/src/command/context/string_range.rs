use std::ops::Range;

use crate::command::string_reader::StringReader;

/// 表示一个范围，实际对应字符串中
/// 从 `start` 到 `end` 字节索引之间的子串。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringRange {
    pub start: usize,
    pub end: usize,
}

impl StringRange {
    /// 构造一个新的子串范围，索引包含 `start`，不包含 `end`。
    #[must_use]
    pub const fn between(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// 在索引 `pos` 处字符的左侧构造一个空范围。
    #[must_use]
    pub const fn at(pos: usize) -> Self {
        Self::between(pos, pos)
    }

    /// 构造一个涵盖两个 [`StringRange`] 的新子串范围，
    /// 返回的新范围同时覆盖这两个范围。
    #[must_use]
    pub fn encompass(a: Self, b: Self) -> Self {
        Self::between(a.start.min(b.start), a.end.max(b.end))
    }

    /// 从该范围获取绑定到 [`StringReader`] 的 [`str`] 子串切片。
    #[must_use]
    pub fn slice_from_reader<'a>(&self, reader: &'a StringReader) -> &'a str {
        &reader.string()[self.start..self.end]
    }

    /// 从该范围获取绑定到 [`String`] 的 [`str`] 子串切片。
    #[must_use]
    pub fn substring_slice<'a>(&self, string: &'a str) -> &'a str {
        &string[self.start..self.end]
    }

    /// 返回该范围的长度是否为零。
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// 返回该范围的长度。
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
