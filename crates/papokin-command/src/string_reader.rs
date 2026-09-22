use crate::errors::command_syntax_error::{
    CommandSyntaxError, CommandSyntaxErrorContext, ContextProvider,
};
use crate::errors::error_types::{self, CommandErrorType};
use papokin_util::text::TextComponent;
use std::borrow::Cow;
use std::str::FromStr;

/// 一种可以逐字符读取字符串供命令使用的结构。
///
/// 它内部使用游标来读取它们，
/// 对确定原因的位置非常重要
/// 由该解析器产生的语法错误。
#[derive(Clone, Debug, Default)]
pub struct StringReader<'a> {
    string: Cow<'a, str>,
    byte_cursor: usize,
}

const SYNTAX_ESCAPE: char = '\\';
const SYNTAX_SINGLE_QUOTE: char = '\'';
const SYNTAX_DOUBLE_QUOTE: char = '"';

impl<'a> StringReader<'a> {
    ///返回一个新的 [`StringReader`] 实例
    /// 从借用的或拥有的字符串创建，并带有光标
    /// 初始时被设为起点。
    pub fn new<S>(string: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        Self {
            string: string.into(),
            byte_cursor: 0,
        }
    }

    ///返回对用于解析的内部字符串的引用。
    #[must_use]
    pub fn string(&self) -> &str {
        &self.string
    }

    ///返回内部光标的当前字节位置。
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.byte_cursor
    }

    /// 设置内部游标的当前字节位置。
    pub const fn set_cursor(&mut self, cursor: usize) {
        self.byte_cursor = cursor;
    }

    /// 获取内部字符串的总字节长度。
    #[must_use]
    pub fn total_length(&self) -> usize {
        self.string.len()
    }

    /// 获取内部字符串中尚未读取的字节长度。
    #[must_use]
    pub fn remaining_length(&self) -> usize {
        self.string.len() - self.byte_cursor
    }

    /// 获取内部字符串中已读取的部分。
    #[must_use]
    pub fn read_part(&self) -> &str {
        &self.string[0..self.byte_cursor]
    }

    /// 获取内部字符串中尚未读取的部分。
    #[must_use]
    pub fn remaining_part(&self) -> &str {
        &self.string[self.byte_cursor..]
    }

    /// 返回读取器是否还能再读取 `length` 个字节
    /// 而不会越界。
    #[must_use]
    pub fn can_read_bytes(&self, length: usize) -> bool {
        self.byte_cursor + length <= self.string.len()
    }

    /// 返回读取器是否还能再读取 1 个字节
    /// 而不会越界。
    #[must_use]
    pub fn can_read_byte(&self) -> bool {
        self.can_read_bytes(1)
    }

    /// 返回读取器是否还能再读取 `length` 个 [`char`]
    /// 而不会越界。
    #[must_use]
    pub fn can_read_chars(&self, length: usize) -> bool {
        self.string[self.byte_cursor..].chars().take(length).count() == length
    }

    /// 返回读取器是否还能再读取 1 个 [`char`]
    /// 而不会越界。
    #[must_use]
    pub fn can_read_char(&self) -> bool {
        self.can_read_chars(1)
    }

    /// 预览光标所在处的字节，但不前进。
    #[must_use]
    pub fn peek_byte(&self) -> Option<u8> {
        self.byte_at(self.byte_cursor)
    }

    /// 在指定的字节索引处获取一个字节。
    #[must_use]
    pub fn byte_at(&self, i: usize) -> Option<u8> {
        self.string.as_bytes().get(i).copied()
    }

    /// 预览光标所在处的 [`char`]，但不前进。
    #[must_use]
    pub fn peek(&self) -> Option<char> {
        self.string[self.byte_cursor..].chars().next()
    }

    /// 以偏移量预览光标所在处的 [`char`]，
    /// 以字节为单位指定，且不前进。
    #[must_use]
    pub fn peek_with_offset(&self, offset: usize) -> Option<char> {
        if self.byte_cursor + offset > self.string.len() {
            return None;
        }
        self.string[(self.byte_cursor + offset)..].chars().next()
    }

    /// 在推进之前，先读取光标所在处的 [`char`]。
    #[must_use = "to skip a character use `skip()` instead"]
    pub fn read(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.byte_cursor += c.len_utf8();
        Some(c)
    }

    /// 跳过游标当前所在处的 [`char`]。
    pub fn skip(&mut self) {
        if let Some(c) = self.peek() {
            self.byte_cursor += c.len_utf8();
        }
    }

    /// 给定的 [`char`] 是否允许出现在数字中。
    #[must_use]
    pub const fn is_allowed_in_number(c: char) -> bool {
        matches!(c, '0'..='9' | '.' | '-')
    }

    /// 给定的 [`char`] 是否可作为带引号字符串的开头和结尾。
    #[must_use]
    pub const fn is_allowed_as_quoted_string_start_end(c: char) -> bool {
        matches!(c, SYNTAX_SINGLE_QUOTE | SYNTAX_DOUBLE_QUOTE)
    }

    /// 给定的 [`char`] 是否允许出现在不带引号的字符串中。
    #[must_use]
    pub const fn is_allowed_in_unquoted_string(c: char) -> bool {
        matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z' | '_' | '-' | '.' | '+')
    }

    /// 跳过任意空白字符，直到没有更多可跳过为止。
    pub fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.skip();
            } else {
                break;
            }
        }
    }

    /// 解析给定的类型 `T`。
    fn read_and_parse<T: FromStr>(
        &mut self,
        expected_error_type: &'static CommandErrorType<0>,
        invalid_error_type: &'static CommandErrorType<1>,
    ) -> Result<T, CommandSyntaxError> {
        let start = self.byte_cursor;
        while let Some(c) = self.peek() {
            if Self::is_allowed_in_number(c) {
                self.skip();
            } else {
                break;
            }
        }
        let result = &self.string[start..self.byte_cursor];
        if result.is_empty() {
            return Err(expected_error_type.create(self));
        }
        if let Ok(value) = result.parse::<T>() {
            Ok(value)
        } else {
            self.byte_cursor = start;
            Err(invalid_error_type.create(self, TextComponent::text(result.to_owned())))
        }
    }

    /// 解析一个 [`bool`]，失败时产生 [`CommandSyntaxError`]。
    pub fn read_bool(&mut self) -> Result<bool, CommandSyntaxError> {
        let start = self.byte_cursor;
        let value = self.read_string()?;
        match value.as_str() {
            "" => Err(error_types::READER_EXPECTED_BOOL.create(self)),
            "true" => Ok(true),
            "false" => Ok(false),
            _ => {
                self.byte_cursor = start;
                Err(error_types::READER_INVALID_BOOL
                    .create(self, TextComponent::text(value.clone())))
            }
        }
    }

    /// 解析一个 [`i32`]，失败时产生 [`CommandSyntaxError`]。
    pub fn read_int(&mut self) -> Result<i32, CommandSyntaxError> {
        self.read_and_parse(
            &error_types::READER_EXPECTED_INT,
            &error_types::READER_INVALID_INT,
        )
    }

    /// 解析一个 [`i64`]，失败时产生 [`CommandSyntaxError`]。
    pub fn read_long(&mut self) -> Result<i64, CommandSyntaxError> {
        self.read_and_parse(
            &error_types::READER_EXPECTED_LONG,
            &error_types::READER_INVALID_LONG,
        )
    }

    /// 解析一个 [`f32`]，失败时产生 [`CommandSyntaxError`]。
    pub fn read_float(&mut self) -> Result<f32, CommandSyntaxError> {
        self.read_and_parse(
            &error_types::READER_EXPECTED_FLOAT,
            &error_types::READER_INVALID_FLOAT,
        )
    }

    /// 解析一个 [`f64`]，失败时产生 [`CommandSyntaxError`]。
    pub fn read_double(&mut self) -> Result<f64, CommandSyntaxError> {
        self.read_and_parse(
            &error_types::READER_EXPECTED_DOUBLE,
            &error_types::READER_INVALID_DOUBLE,
        )
    }

    /// 读取不带引号的字符串（不由引号包围）
    pub fn read_unquoted_string(&mut self) -> String {
        let start = self.byte_cursor;
        while let Some(c) = self.peek() {
            if Self::is_allowed_in_unquoted_string(c) {
                self.skip();
            } else {
                break;
            }
        }
        self.string[start..self.byte_cursor].to_string()
    }

    /// 读取任意字符串，无论其带引号还是不带引号。
    pub fn read_string(&mut self) -> Result<String, CommandSyntaxError> {
        let Some(next) = self.peek() else {
            return Ok(String::new());
        };
        if Self::is_allowed_as_quoted_string_start_end(next) {
            self.skip();
            self.read_string_until(next)
        } else {
            Ok(self.read_unquoted_string())
        }
    }

    /// 读取带引号的字符串（由引号包围）
    pub fn read_quoted_string(&mut self) -> Result<String, CommandSyntaxError> {
        let Some(next) = self.peek() else {
            return Ok(String::new());
        };
        if Self::is_allowed_as_quoted_string_start_end(next) {
            self.skip();
            self.read_string_until(next)
        } else {
            Err(error_types::READER_EXPECTED_START_QUOTE.create(self))
        }
    }

    /// 读取字符串，直到遇到给定字符。
    pub fn read_string_until(&mut self, terminator: char) -> Result<String, CommandSyntaxError> {
        let mut result: String = String::new();
        let mut escaped: bool = false;
        while let Some(c) = self.peek() {
            if escaped {
                if c == terminator || c == SYNTAX_ESCAPE {
                    result.push(c);
                    self.skip();
                    escaped = false;
                } else {
                    return Err(error_types::READER_INVALID_ESCAPE
                        .create(self, TextComponent::text(c.to_string())));
                }
            } else {
                self.skip();
                if c == SYNTAX_ESCAPE {
                    escaped = true;
                } else if c == terminator {
                    return Ok(result);
                } else {
                    result.push(c);
                }
            }
        }
        Err(error_types::READER_EXPECTED_END_QUOTE.create(self))
    }

    /// 期望消费一个特定的 [`char`]，否则返回 [`Err`]。
    pub fn expect(&mut self, c: char) -> Result<(), CommandSyntaxError> {
        if self.peek() == Some(c) {
            self.skip();
            Ok(())
        } else {
            Err(error_types::READER_EXPECTED_SYMBOL
                .create(self, TextComponent::text(c.to_string())))
        }
    }

    /// 持续跳过读取器中的字符，直到遇到空格或字符串末尾。
    pub fn read_until_space(&mut self) {
        while !matches!(self.peek(), None | Some(' ')) {
            self.skip();
        }
    }

    /// 将此读取器转换为 `'static` 形式，从而
    /// 可用于对读取器进行快照。
    #[must_use]
    pub fn into_owned(self) -> StringReader<'static> {
        StringReader {
            string: Cow::Owned(self.string.into_owned()),
            byte_cursor: self.byte_cursor,
        }
    }

    /// 将此读取器克隆为 `'static` 形式，该形式
    /// 可用于对读取器进行快照。
    #[must_use]
    pub fn clone_into_owned(&self) -> StringReader<'static> {
        StringReader {
            string: Cow::Owned(self.string.to_string()),
            byte_cursor: self.byte_cursor,
        }
    }
}

impl ContextProvider for StringReader<'_> {
    fn context(&self) -> CommandSyntaxErrorContext {
        CommandSyntaxErrorContext {
            input: self.string.to_string(),
            cursor: self.byte_cursor,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{errors::error_types, string_reader::StringReader};

    #[test]
    fn non_parsing_methods() {
        let mut reader = StringReader::new("hello 🎃! ");
        assert_eq!(reader.read(), Some('h'));

        assert_eq!(reader.peek(), Some('e'));
        assert_eq!(reader.read(), Some('e'));

        assert!(reader.can_read_char());
        assert!(reader.can_read_chars(7));
        assert!(!reader.can_read_chars(8));

        reader.expect('l').expect("预期为 'l'");
        reader.skip();
        reader.expect('o').expect("预期为 'o'");

        // Note: 🎃 在 UTF-8 中占 4 字节
        assert!(reader.can_read_bytes(6));
        assert_eq!(reader.remaining_length(), 7);
        assert!(!reader.can_read_bytes(8));

        assert_eq!(reader.read_part(), "hello");
        reader.set_cursor(6);
        assert_eq!(reader.remaining_part(), "🎃! ");
        assert_eq!(reader.peek_byte(), Some(0xF0));
        assert_eq!(reader.read(), Some('🎃'));

        reader.skip_whitespace(); // 应为无操作（NO-OP）
        assert_eq!(reader.read(), Some('!'));

        reader.skip_whitespace();
        assert_ne!(reader.expect(' '), Ok(()));
    }

    #[test]
    fn read_types() {
        let mut reader =
            StringReader::new("12 34  7890123456     1.233   1.592394582  false'false' faux");
        assert_eq!(reader.read_int(), Ok(12));
        reader.skip_whitespace();
        assert_eq!(reader.read_long(), Ok(34));
        reader.skip_whitespace();

        assert!(
            reader
                .read_int()
                .unwrap_err()
                .is(&error_types::READER_INVALID_INT)
        );
        reader.skip_whitespace();
        assert_eq!(reader.read_long(), Ok(7890123456));
        reader.skip_whitespace();

        assert!((reader.read_float().expect("预期为浮点数") - 1.233f32).abs() < 1e-07);
        reader.skip_whitespace();

        assert!((reader.read_double().expect("预期为双精度浮点数") - 1.592394582f64).abs() < 1e-15);
        reader.skip_whitespace();

        assert_eq!(reader.read_bool(), Ok(false));
        assert_eq!(reader.read_bool(), Ok(false));
        reader.skip_whitespace();
        assert_ne!(reader.read_bool(), Ok(false));
    }

    #[test]
    fn read_strings() {
        let mut reader = StringReader::new("'apple' banana orange \"orange\" 'hello\"");

        assert_eq!(reader.read_quoted_string(), Ok("apple".to_string()));
        reader.skip_whitespace();
        assert_eq!(reader.read_unquoted_string(), "banana".to_string());
        reader.skip_whitespace();

        assert_eq!(reader.read_string(), Ok("orange".to_string()));
        reader.skip_whitespace();
        assert_eq!(reader.read_string(), Ok("orange".to_string()));
        reader.skip_whitespace();

        assert!(
            reader
                .read_quoted_string()
                .unwrap_err()
                .is(&error_types::READER_EXPECTED_END_QUOTE)
        );
    }
}
