use crate::context::string_range::StringRange;
use crate::suggestion::{Suggestion, SuggestionText};
use papokin_util::text::TextComponent;
use std::borrow::Borrow;
use std::cmp::Ordering;

/// 表示 [`Suggestion`] 的构建器。
pub struct SuggestionsBuilder {
    /// 表示 [`SuggestionsBuilder`] 的起始位置
    /// 从输入字符串的开头开始。
    pub start: usize,

    /// 表示 [`SuggestionsBuilder`] 的输入。
    pub input: String,

    /// 表示 [`SuggestionsBuilder`] 输入的小写形式。
    pub input_lowercase: String,

    /// 此 [`SuggestionsBuilder`] 的最终结果。
    pub result: Vec<Suggestion>,
}

impl SuggestionsBuilder {
    /// 从给定的内容构造一个新的 [`SuggestionsBuilder`]
    /// 输入字符串以及相对于它的起始位置。
    #[must_use]
    pub fn new(input: &str, start: usize) -> Self {
        Self {
            input: input.to_string(),
            input_lowercase: input.to_lowercase(),
            start,
            result: Vec::new(),
        }
    }

    /// 获取底层输入字符串的剩余子串。
    #[must_use]
    pub fn remaining(&self) -> &str {
        &self.input[self.start.min(self.input.len())..]
    }

    /// 获取底层小写输入字符串的剩余子串。
    #[must_use]
    pub fn remaining_lowercase(&self) -> &str {
        &self.input_lowercase[self.start.min(self.input_lowercase.len())..]
    }

    /// 构建 [`Suggestions`] 对象，过程中消耗自身。
    #[must_use]
    pub fn build(self) -> Suggestions {
        Suggestions::create(&self.input, self.result)
    }

    /// 向此构建器添加一条不带提示框的建议。
    #[must_use]
    pub fn suggest<T>(mut self, text: T) -> Self
    where
        T: Into<SuggestionText>,
    {
        let text = text.into();
        if text.cached_text() != self.remaining() {
            self.result.push(Suggestion::without_tooltip(
                StringRange::between(self.start, self.input.len()),
                text,
            ));
        }
        self
    }

    /// 向此构建器添加一条带提示框的建议。
    #[must_use]
    pub fn suggest_with_tooltip<T>(mut self, text: T, tooltip: TextComponent) -> Self
    where
        T: Into<SuggestionText>,
    {
        let text = text.into();
        if text.cached_text() != self.remaining() {
            self.result.push(Suggestion::with_tooltip(
                StringRange::between(self.start, self.input.len()),
                text,
                tooltip,
            ));
        }
        self
    }

    /// 将另一个 [`SuggestionsBuilder`] 的所有建议添加到此构建器。
    #[must_use]
    pub fn append(mut self, other: Self) -> Self {
        for suggestion in other.result {
            self.result.push(suggestion);
        }
        self
    }

    /// 基于当前对象创建另一个 [`SuggestionsBuilder`]
    /// 通过复制输入并取用起始位置。
    #[must_use]
    pub fn create_offset(&self, start: usize) -> Self {
        Self {
            input: self.input.clone(),
            input_lowercase: self.input_lowercase.clone(),
            start,
            result: Vec::new(),
        }
    }

    /// 只取满足当前构建器前缀的值，并
    /// 会对它们给出建议。当前要使此函数正常工作，**提供的所有值
    /// 必须为小写**。
    ///
    /// 示例：
    /// - 若构建器拥有 `b`，且值为 `acacia_boat`、`blue` 和 `stick`，则只有前两个会被计数，
    ///   例如 `boat` 和 `blue` 都以字母 `b` 开头。
    /// - 若构建器拥有的是 `bl`，则只有 `blue` 会被计数。
    #[must_use]
    pub fn filter_and_suggest_lowercase(mut self, values: Vec<String>) -> Self {
        for value in values {
            if Self::matches_substr(self.remaining_lowercase(), &value) {
                self = self.suggest(value);
            }
        }
        self
    }

    /// 只取满足当前构建器前缀的值，并
    /// 会对它们给出建议。
    ///
    /// 示例：
    /// - 若构建器拥有 `b`，且值为 `acacia_boat`、`blue` 和 `stick`，则只有前两个会被计数，
    ///   例如 `boat` 和 `blue` 都以字母 `b` 开头。
    /// - 若构建器拥有的是 `bl`，则只有 `blue` 会被计数。
    #[must_use]
    pub fn filter_and_suggest(mut self, values: &[&str]) -> Self {
        for value in values {
            if Self::matches_substr(self.remaining_lowercase(), &value.to_lowercase()) {
                self = self.suggest(value.to_string());
            }
        }
        self
    }

    /// 仅当值满足当前构建器前缀时才取走该值，并
    /// 会对它们给出建议。
    #[must_use]
    pub fn filter_and_suggest_one(mut self, value: impl Into<SuggestionText>) -> Self {
        let value = value.into();
        if Self::matches_substr(
            self.remaining_lowercase(),
            &value.cached_text().to_lowercase(),
        ) {
            self = self.suggest(value);
        }
        self
    }

    /// 只取满足当前构建器前缀的值，并
    /// 会对它们给出建议。
    ///
    /// 示例：
    /// - 若构建器拥有 `b`，且值为 `acacia_boat`、`blue` 和 `stick`，则只有前两个会被计数，
    ///   例如 `boat` 和 `blue` 都以字母 `b` 开头。
    /// - 若构建器拥有的是 `bl`，则只有 `blue` 会被计数。
    #[must_use]
    pub fn filter_and_suggest_iter(
        mut self,
        values: impl IntoIterator<Item = impl Into<SuggestionText>>,
    ) -> Self {
        for value in values {
            let value = value.into();
            if Self::matches_substr(
                self.remaining_lowercase(),
                &value.cached_text().to_lowercase(),
            ) {
                self = self.suggest(value);
            }
        }
        self
    }

    fn matches_substr(pattern: &str, input: &str) -> bool {
        let mut current_str = input;
        while !current_str.starts_with(pattern) {
            match current_str.find(['.', '_', '/', ':']) {
                Some(pos) => current_str = &current_str[(pos + 1)..],
                None => return false,
            }
        }
        true
    }

    /// 一个用于建议坐标相关文本的辅助方法。
    pub fn suggest_3d_coordinates(
        mut self,
        suggestions: TextCoordinates,
        validator: impl Fn(&str) -> bool,
    ) -> Suggestions {
        let input = self.remaining();
        let coordinate = suggestions.get_coordinate();

        if input.is_empty() {
            let full = format!("{coordinate} {coordinate} {coordinate}");
            if validator(&full) {
                self = self.filter_and_suggest_one(coordinate.to_string());
                self = self.filter_and_suggest_one(format!("{coordinate} {coordinate}"));
                self = self.filter_and_suggest_one(full);
            }
        } else {
            let mut split = input.split(' ');

            match (split.next(), split.next(), split.next()) {
                (Some(part1), None, None) => {
                    let full = format!("{part1} {coordinate} {coordinate}");
                    if validator(&full) {
                        let partial = format!("{part1} {coordinate}");
                        self = self.filter_and_suggest_one(partial);
                        self = self.filter_and_suggest_one(full);
                    }
                }
                (Some(part1), Some(part2), None) => {
                    let full = format!("{part1} {part2} {coordinate}");
                    if validator(&full) {
                        self = self.filter_and_suggest_one(full);
                    }
                }
                _ => {}
            }
        }

        self.build()
    }

    /// 一个用于建议坐标相关文本的辅助方法。
    pub fn suggest_2d_coordinates(
        mut self,
        suggestions: TextCoordinates,
        validator: impl Fn(&str) -> bool,
    ) -> Suggestions {
        let input = self.remaining();
        let coordinate = suggestions.get_coordinate();

        if input.is_empty() {
            let full = format!("{coordinate} {coordinate}");
            if validator(&full) {
                self = self.filter_and_suggest_one(coordinate.to_string());
                self = self.filter_and_suggest_one(full);
            }
        } else {
            let mut split = input.split(' ');

            if let Some(part) = split.next()
                && split.next().is_none()
            {
                let full = format!("{part} {coordinate}");
                if validator(&full) {
                    self = self.filter_and_suggest_one(full);
                }
            }
        }

        self.build()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Suggestions {
    pub range: StringRange,
    pub suggestions: Vec<Suggestion>,
}

impl Suggestions {
    /// 从给定内容构造新的 [`Suggestions`] 结构
    /// 一个范围和若干 [`Suggestion`]。
    #[must_use]
    pub const fn new(range: StringRange, suggestions: Vec<Suggestion>) -> Self {
        Self { range, suggestions }
    }

    /// 构造一个大小为零且没有范围的新 [`Suggestions`]。
    #[must_use]
    pub const fn empty() -> Self {
        Self::new(StringRange::at(0), vec![])
    }

    /// 返回此 [`Suggestions`] *是否* 为零大小。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.suggestions.is_empty()
    }

    /// 将随命令提供的所有 [`Suggestions`] 合并为单个 [`Suggestions`]。
    #[must_use]
    pub fn merge<I, S>(command: &str, input: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Borrow<Self>,
    {
        let input: Vec<S> = input.into_iter().collect();

        if input.is_empty() {
            return Self::empty();
        } else if input.len() == 1 {
            return input[0].borrow().clone();
        }

        let mut texts = Vec::new();

        for suggestions in &input {
            for suggestion in &suggestions.borrow().suggestions {
                if !texts.contains(&suggestion) {
                    texts.push(suggestion);
                }
            }
        }

        Self::create(command, texts)
    }

    /// 从当前状态创建一个单独的 [`Suggestions`] 结构，
    /// 多个 [`Suggestion`] 和一条命令。
    #[must_use]
    pub fn create<I, S>(command: &str, suggestions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Borrow<Suggestion>,
    {
        let suggestions: Vec<S> = suggestions.into_iter().collect();

        if suggestions.is_empty() {
            return Self::empty();
        }

        // 首先，我们确定涵盖所提供全部建议的范围
        let range = suggestions
            .iter()
            .map(|s| s.borrow().range)
            .reduce(StringRange::encompass)
            .expect("补全建议列表非空，因此范围应当存在");

        let mut texts = Vec::new();
        for suggestion in &suggestions {
            let suggestion = suggestion.borrow().expand(command, range);
            if !texts.contains(&suggestion) {
                texts.push(suggestion);
            }
        }

        Self::new(range, Self::sort(texts))
    }

    /// 按以下优先级对一组 [`Suggestion`] 进行排序：
    ///
    /// 1. 如果两个建议都是整数，则比较它们的整数值。
    /// 2. 否则，按字典序比较它们的文本。
    fn sort(suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
        enum PushSide {
            Text,
            Integer,
            Break,
        }

        let mut text_suggestions = Vec::new();
        let mut integer_suggestions = Vec::new();

        let len = suggestions.len();

        for suggestion in suggestions {
            match suggestion.text {
                SuggestionText::Text(text) => {
                    let text_lowercase = text.to_lowercase();
                    text_suggestions.push((
                        text,
                        suggestion.tooltip,
                        suggestion.range,
                        text_lowercase,
                    ));
                }
                SuggestionText::Integer { cached_text, value } => integer_suggestions.push((
                    cached_text,
                    value,
                    suggestion.tooltip,
                    suggestion.range,
                )),
            }
        }

        text_suggestions.sort_by(|a, b| a.3.cmp(&b.3));
        integer_suggestions.sort_unstable_by_key(|x| x.1);

        let mut text_iter = text_suggestions.into_iter().peekable();
        let mut integer_iter = integer_suggestions.into_iter().peekable();

        let mut suggestions = Vec::with_capacity(len);

        loop {
            let text = text_iter.peek();
            let integer = integer_iter.peek();

            let side = match (text, integer) {
                (Some(text), Some(integer)) => match text.0.cmp(&integer.0) {
                    Ordering::Less => PushSide::Text,
                    Ordering::Greater => PushSide::Integer,
                    Ordering::Equal => {
                        tracing::error!("合并时发现重复的补全建议");
                        PushSide::Text
                    }
                },
                (Some(_), None) => PushSide::Text,
                (None, Some(_)) => PushSide::Integer,
                (None, None) => PushSide::Break,
            };

            match side {
                PushSide::Text => {
                    if let Some(text) = text_iter.next() {
                        suggestions.push(Suggestion {
                            text: SuggestionText::Text(text.0),
                            tooltip: text.1,
                            range: text.2,
                        });
                    }
                }
                PushSide::Integer => {
                    if let Some(text) = integer_iter.next() {
                        suggestions.push(Suggestion {
                            text: SuggestionText::Integer {
                                cached_text: text.0,
                                value: text.1,
                            },
                            tooltip: text.2,
                            range: text.3,
                        });
                    }
                }
                PushSide::Break => break,
            }
        }

        suggestions
    }
}

/// 表示仅限服务器端的坐标建议。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextCoordinates {
    /// 表示 `^ ^ ^`。
    Local,

    /// 表示 `~ ~ ~`。
    Global,
}

impl TextCoordinates {
    /// 获取此建议集某个坐标的符号。
    #[must_use]
    pub const fn get_coordinate(self) -> &'static str {
        match self {
            Self::Local => "^",
            Self::Global => "~",
        }
    }
}
