mod markers;
mod operations;
mod rules;

#[cfg(test)]
mod tests;

use crate::errors::command_syntax_error::{CommandSyntaxError, CommandSyntaxErrorContext};
use crate::errors::error_types::{CommandErrorType, LITERAL_INCORRECT};
use crate::parser::{Parser, ParserErrors};
use crate::snbt::markers::{ArrayPrefix, Base, IntegerLiteral, Sign, SignedPrefix, TypeSuffix};
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_codecs::Number;
use papokin_data::translation;
use papokin_nbt::tag::NbtTag;
use papokin_util::text::TextComponent;

pub const NUMBER_PARSE_FAILURE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::SNBT_PARSER_NUMBER_PARSE_FAILURE);

pub const UNDERSCORE_NOT_ALLOWED: CommandErrorType<0> =
    CommandErrorType::new(translation::java::SNBT_PARSER_UNDESCORE_NOT_ALLOWED);

pub const EXPECTED_HEX_ESCAPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::SNBT_PARSER_EXPECTED_HEX_ESCAPE);

pub const EXPECTED_NON_NEGATIVE_NUMBER: CommandErrorType<0> =
    CommandErrorType::new(translation::java::SNBT_PARSER_EXPECTED_NON_NEGATIVE_NUMBER);

pub const INVALID_ARRAY_ELEMENT_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::SNBT_PARSER_INVALID_ARRAY_ELEMENT_TYPE);

pub const EXPECTED_INTEGER_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::SNBT_PARSER_EXPECTED_INTEGER_TYPE);

/// 一种解析 SNBT 的结构。
///
/// 此结构保存一个读取器，并给出最远位置的错误或建议
/// 以修复解析过程中曾出现过的错误。
pub struct SnbtParser<'r, 's> {
    reader: &'r mut StringReader<'s>,
    errors: ParserErrors,
    /// 复合/列表互递归的嵌套深度计数，用于拒绝深嵌套输入
    depth: usize,
}

//
// 用法
//
impl SnbtParser<'_, '_> {
    /// 使用给定的 [`StringReader`] 解析 SNBT，返回解析的结果或错误。
    pub fn parse_for_commands(reader: &mut StringReader) -> Result<NbtTag, CommandSyntaxError> {
        let (result, errors) = {
            let mut parser = SnbtParser {
                reader,
                errors: ParserErrors::default(),
                depth: 0,
            };

            let literal = parser.parse();
            let errors = parser.errors;

            (literal, errors)
        };

        result.ok_or_else(|| {
            if let Some(error) = errors.command_error {
                CommandSyntaxError {
                    error_type: error.error_type,
                    message: TextComponent::translate(
                        error.java_translation_key,
                        error
                            .arguments
                            .into_iter()
                            .map(TextComponent::text)
                            .collect::<Vec<_>>(),
                    ),
                    context: Some(CommandSyntaxErrorContext {
                        input: reader.string().to_string(),
                        cursor: errors.cursor,
                    }),
                }
            } else {
                // 这不应该发生……如果解析未成功，应当会有一个错误来补充说明。
                // 报告错误的取巧办法：
                const PARSING_FAILED_WITHOUT_ERRORS: CommandErrorType<0> =
                    CommandErrorType::new(translation::java::COMMAND_FAILED);
                tracing::error!(
                    "SNBT 解析失败，但没有任何可报告的错误（请向 Pumpkin 报告此问题）：{}",
                    reader.string()
                );
                PARSING_FAILED_WITHOUT_ERRORS.create(reader)
            }
        })
    }

    // 使用给定的 [`StringReader`] 解析 SNBT，并给出修复解析错误的建议。
    #[must_use]
    pub fn parse_for_suggestions(mut builder: SuggestionsBuilder) -> Suggestions {
        let errors = {
            let mut reader = StringReader::new(&builder.input);
            reader.set_cursor(builder.start);

            let mut parser = SnbtParser {
                reader: &mut reader,
                errors: ParserErrors::default(),
                depth: 0,
            };

            let _ = parser.parse();
            parser.errors
        };

        if !errors.suggestions.is_empty() {
            builder = builder.create_offset(errors.cursor);
            for suggestion in &errors.suggestions {
                builder = builder.filter_and_suggest_one(suggestion.to_string());
            }
        }

        builder.build()
    }
}

//
// 辅助函数
//
impl SnbtParser<'_, '_> {
    /// 解析整数类型后缀的工具方法。
    fn integer_type_suffix(&mut self) -> Option<TypeSuffix> {
        self.reader.skip_whitespace();
        match self.reader.peek() {
            Some('b' | 'B') => {
                self.reader.skip();
                Some(TypeSuffix::Byte)
            }
            Some('s' | 'S') => {
                self.reader.skip();
                Some(TypeSuffix::Short)
            }
            Some('i' | 'I') => {
                self.reader.skip();
                Some(TypeSuffix::Int)
            }
            Some('l' | 'L') => {
                self.reader.skip();
                Some(TypeSuffix::Long)
            }
            _ => {
                // 仅给出 b|B 作为错误，它是第一个出错的选项。
                self.store_dynamic_error_and_suggest(
                    &LITERAL_INCORRECT,
                    "b|B",
                    &["b", "B", "s", "S", "i", "I", "l", "L"],
                );
                None
            }
        }
    }

    /// 解析特定进制整数的通用方法。
    fn parse_numeral(&mut self, base: Base) -> Option<String> {
        self.parse_or_revert(|parser| {
            parser.reader.skip_whitespace();
            let slice = parser.reader.string();

            let start = parser.reader.cursor();

            let mut end = start;
            for (i, c) in slice[start..].char_indices() {
                if !base.should_allow(c) {
                    break;
                }
                end = start + i + c.len_utf8();
            }

            if start == end {
                parser.store_simple_error(base.no_value_error_type());
                None
            } else if slice.as_bytes()[start] == b'_' || slice.as_bytes()[end - 1] == b'_' {
                parser.store_simple_error(&UNDERSCORE_NOT_ALLOWED);
                None
            } else {
                parser.reader.set_cursor(end);
                Some(parser.reader.string()[start..end].to_string())
            }
        })
    }

    /// 解析一个值，如果失败则回退到初始状态。
    #[inline]
    fn parse_or_revert<T>(&mut self, closure: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let start = self.reader.cursor();
        let result = closure(self);
        if result.is_none() {
            self.reader.set_cursor(start);
        }
        result
    }

    /// 将 `reference` 切片中除 `_` 之外的每个字符追加到给定的 `buffer` 中。
    fn clean_and_append(buffer: &mut String, reference: &str) {
        // 这里确实还能进一步优化
        // 以字节而非字符处理，但那样
        // 可能需要 unsafe 代码。值得吗？
        // TODO
        for c in reference.chars() {
            if c != '_' {
                buffer.push(c);
            }
        }
    }

    /// 贪婪地解析特定数量十六进制数字的通用方法（不允许下划线）。
    fn hex_literal(&mut self, digits: usize) -> Option<String> {
        self.parse_or_revert(|parser| {
            parser.reader.skip_whitespace();
            let slice = parser.reader.string();

            let start = parser.reader.cursor();

            let mut end = start;
            for (count, (i, c)) in slice[start..].char_indices().enumerate() {
                if count == digits || !c.is_ascii_hexdigit() {
                    break;
                }
                end = start + i + c.len_utf8();
            }

            if end - start < digits {
                parser.store_dynamic_error(&EXPECTED_HEX_ESCAPE, digits.to_string());
                None
            } else {
                parser.reader.set_cursor(end);
                Some(parser.reader.string()[start..end].to_string())
            }
        })
    }

    fn repeated_with_trailing_comma<T, S>(
        &mut self,
        rule: impl Fn(&mut Self) -> Option<T>,
        new: S,
        insert: impl Fn(&mut S, T),
    ) -> S {
        let mut elements = new;
        let mut first = true;

        loop {
            if !first {
                let parse_comma = self.parse_or_revert(|parser| {
                    parser.reader.skip_whitespace();
                    if parser.reader.peek() == Some(',') {
                        parser.reader.skip();
                        Some(())
                    } else {
                        parser.store_dynamic_error_and_suggest(&LITERAL_INCORRECT, ",", &[","]);
                        None
                    }
                });
                if parse_comma.is_none() {
                    break;
                }
            }

            if let Some(parsed) = self.parse_or_revert(&rule) {
                insert(&mut elements, parsed);
            } else {
                break;
            }

            first = false;
        }

        elements
    }

    fn repeated_with_trailing_comma_vec<T>(
        &mut self,
        rule: impl Fn(&mut Self) -> Option<T>,
    ) -> Vec<T> {
        self.repeated_with_trailing_comma(rule, Vec::new(), Vec::push)
    }

    fn parse_integer_literal(
        &mut self,
        literal: &IntegerLiteral,
        suffix: TypeSuffix,
    ) -> Option<Number> {
        let signed = literal.get_signed_prefix_or_default() == SignedPrefix::Signed;
        if !signed && literal.sign == Sign::Minus {
            self.store_simple_error(&EXPECTED_NON_NEGATIVE_NUMBER);
            return None;
        }

        let mut number =
            String::with_capacity(literal.digits.len() + literal.sign.minimum_size_parsable());

        literal.sign.append_minimum_str_parsable(&mut number);
        Self::clean_and_append(&mut number, &literal.digits);

        let radix = literal.base.radix();

        // 错误消息差别很大，以便与 Java 版的错误消息保持一致。
        match (signed, suffix) {
            (true, TypeSuffix::Byte) => {
                let integer = self.parse_int_or_error(&number, radix)?;

                integer.try_into().map_or_else(
                    |_| {
                        self.store_dynamic_error(
                            &NUMBER_PARSE_FAILURE,
                            format!("Value out of range. Value:\"{number}\" Radix:{radix}"),
                        );
                        None
                    },
                    |byte| Some(Number::Byte(byte)),
                )
            }
            (true, TypeSuffix::Short) => {
                let integer = self.parse_int_or_error(&number, radix)?;

                integer.try_into().map_or_else(
                    |_| {
                        self.store_dynamic_error(
                            &NUMBER_PARSE_FAILURE,
                            format!("Value out of range. Value:\"{number}\" Radix:{radix}"),
                        );
                        None
                    },
                    |short| Some(Number::Short(short)),
                )
            }
            (true, TypeSuffix::Int) => Some(Number::Int(self.parse_int_or_error(&number, radix)?)),
            (true, TypeSuffix::Long) => i64::from_str_radix(&number, radix).map_or_else(
                |_| {
                    self.store_dynamic_error(
                        &NUMBER_PARSE_FAILURE,
                        format!("For input string: \"{number}\""),
                    );
                    None
                },
                |long| Some(Number::Long(long)),
            ),
            (false, TypeSuffix::Byte) => {
                let integer = self.parse_int_or_error(&number, radix)?;

                TryInto::<u8>::try_into(integer).map_or_else(
                    |_| {
                        self.store_dynamic_error(
                            &NUMBER_PARSE_FAILURE,
                            format!("out of range: {number}"),
                        );
                        None
                    },
                    |byte| Some(Number::Byte(byte as i8)),
                )
            }
            (false, TypeSuffix::Short) => {
                let integer = self.parse_int_or_error(&number, radix)?;

                TryInto::<u16>::try_into(integer).map_or_else(
                    |_| {
                        self.store_dynamic_error(
                            &NUMBER_PARSE_FAILURE,
                            format!("out of range: {number}"),
                        );
                        None
                    },
                    |short| Some(Number::Short(short as i16)),
                )
            }
            (false, TypeSuffix::Int) => u32::from_str_radix(&number, radix).map_or_else(
                |_| {
                    self.store_dynamic_error(
                        &NUMBER_PARSE_FAILURE,
                        format!("String value {number} exceeds range of unsigned int."),
                    );
                    None
                },
                |int| Some(Number::Int(int as i32)),
            ),
            (false, TypeSuffix::Long) => u64::from_str_radix(&number, radix).map_or_else(
                |_| {
                    self.store_dynamic_error(
                        &NUMBER_PARSE_FAILURE,
                        format!("String value {number} exceeds range of unsigned long."),
                    );
                    None
                },
                |long| Some(Number::Long(long as i64)),
            ),
            _ => {
                self.store_simple_error(&EXPECTED_INTEGER_TYPE);
                None
            }
        }
    }

    fn parse_int_or_error(&mut self, number: &str, radix: u32) -> Option<i32> {
        i32::from_str_radix(number, radix).map_or_else(
            |_| {
                self.store_dynamic_error(
                    &NUMBER_PARSE_FAILURE,
                    format!("For input string: \"{number}\""),
                );
                None
            },
            Some,
        )
    }

    fn create_prefixed_array(
        &mut self,
        values: &[IntegerLiteral],
        prefix: ArrayPrefix,
    ) -> Option<NbtTag> {
        match prefix {
            ArrayPrefix::Byte => self.create_byte_array(values),
            ArrayPrefix::Int => self.create_int_array(values),
            ArrayPrefix::Long => self.create_long_array(values),
        }
    }

    fn create_byte_array(&mut self, values: &[IntegerLiteral]) -> Option<NbtTag> {
        let mut bytes = Vec::with_capacity(values.len());
        for value in values {
            if !matches!(value.suffix.1, TypeSuffix::None | TypeSuffix::Byte) {
                self.store_simple_error(&INVALID_ARRAY_ELEMENT_TYPE);
                return None;
            }
            bytes.push(self.parse_integer_literal(value, TypeSuffix::Byte)?.into());
        }
        Some(NbtTag::ByteArray(bytes.into()))
    }

    fn create_int_array(&mut self, values: &[IntegerLiteral]) -> Option<NbtTag> {
        let mut ints = Vec::with_capacity(values.len());
        for value in values {
            let suffix = value.suffix.1.or(TypeSuffix::Int);
            if !matches!(
                suffix,
                TypeSuffix::Byte | TypeSuffix::Short | TypeSuffix::Int
            ) {
                self.store_simple_error(&INVALID_ARRAY_ELEMENT_TYPE);
                return None;
            }
            ints.push(self.parse_integer_literal(value, suffix)?.into());
        }
        Some(NbtTag::IntArray(ints))
    }

    fn create_long_array(&mut self, values: &[IntegerLiteral]) -> Option<NbtTag> {
        let mut longs = Vec::with_capacity(values.len());
        for value in values {
            let suffix = value.suffix.1.or(TypeSuffix::Long);
            if !matches!(
                suffix,
                TypeSuffix::Byte | TypeSuffix::Short | TypeSuffix::Int | TypeSuffix::Long
            ) {
                self.store_simple_error(&INVALID_ARRAY_ELEMENT_TYPE);
                return None;
            }
            longs.push(self.parse_integer_literal(value, suffix)?.into());
        }
        Some(NbtTag::LongArray(longs))
    }
}

impl<'r, 's> Parser<'r, 's> for SnbtParser<'r, 's> {
    fn state_mut(&mut self) -> (&mut StringReader<'s>, &mut ParserErrors) {
        (self.reader, &mut self.errors)
    }
}
