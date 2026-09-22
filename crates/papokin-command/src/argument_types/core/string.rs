use papokin_protocol::java::client::play::StringProtoArgBehavior;

use crate::context::command_context::CommandContext;
use crate::{
    argument_types::argument_type::{ArgumentType, JavaClientArgumentType},
    errors::command_syntax_error::CommandSyntaxError,
    string_reader::StringReader,
};

pub enum StringArgumentType {
    /// 接受单个不带引号的词。
    SingleWord,

    /// 接受带引号或不带引号的字符串。
    QuotablePhrase,

    /// 取走 [`StringReader`] 中的剩余文本并返回。
    GreedyPhrase,
}

impl<S: crate::source::CommandSource> ArgumentType<S> for StringArgumentType {
    type Item = String;

    fn parse(&self, reader: &mut StringReader) -> Result<String, CommandSyntaxError> {
        match self {
            Self::SingleWord => Ok(reader.read_unquoted_string()),
            Self::QuotablePhrase => reader.read_string(),
            Self::GreedyPhrase => {
                let text = reader.remaining_part().to_owned();
                reader.set_cursor(reader.total_length());
                Ok(text)
            }
        }
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::String(match self {
            Self::SingleWord => StringProtoArgBehavior::SingleWord,
            Self::QuotablePhrase => StringProtoArgBehavior::QuotablePhrase,
            Self::GreedyPhrase => StringProtoArgBehavior::GreedyPhrase,
        })
    }

    fn examples(&self) -> Vec<String> {
        match self {
            Self::SingleWord => examples!("word", "words_with_underscores"),
            Self::QuotablePhrase => examples!("\"quoted phrase\"", "word", "\"\""),
            Self::GreedyPhrase => examples!("word", "words with spaces", "\"and symbols\""),
        }
    }
}

impl StringArgumentType {
    ///以字符串切片的形式返回 [`CommandContext`] 中已解析的 `String` 参数。
    pub fn get<'a, S: crate::source::CommandSource>(
        context: &'a CommandContext<S>,
        name: &str,
    ) -> Result<&'a str, CommandSyntaxError> {
        Ok(context.get_argument::<String>(name)?.as_str())
    }
}

#[cfg(test)]
mod test {
    use crate::{
        argument_types::core::string::StringArgumentType, errors::error_types,
        string_reader::StringReader,
    };

    #[test]
    fn parse_single_quoted() {
        let mut reader = StringReader::new("'single-quoted string!'");

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::QuotablePhrase,
            "single-quoted string!".to_owned()
        );

        assert_parse_ok_reset!(&mut reader, StringArgumentType::SingleWord, String::new());

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::GreedyPhrase,
            "'single-quoted string!'".to_owned()
        );
    }

    #[test]
    fn parse_double_quoted() {
        let mut reader = StringReader::new("\"double-quoted string!\"");

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::QuotablePhrase,
            "double-quoted string!".to_owned()
        );

        assert_parse_ok_reset!(&mut reader, StringArgumentType::SingleWord, String::new());

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::GreedyPhrase,
            "\"double-quoted string!\"".to_owned()
        );
    }

    #[test]
    fn parse_identifier() {
        let mut reader = StringReader::new(".i_AM_an-1den+ifier.");

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::QuotablePhrase,
            ".i_AM_an-1den+ifier.".to_owned()
        );

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::SingleWord,
            ".i_AM_an-1den+ifier.".to_owned()
        );

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::GreedyPhrase,
            ".i_AM_an-1den+ifier.".to_owned()
        );
    }

    #[test]
    fn quoted_incorrectly() {
        let mut reader = StringReader::new("'incorrect\"");

        assert_parse_err_reset!(
            &mut reader,
            StringArgumentType::QuotablePhrase,
            &error_types::READER_EXPECTED_END_QUOTE
        );

        assert_parse_ok_reset!(&mut reader, StringArgumentType::SingleWord, String::new());

        assert_parse_ok_reset!(
            &mut reader,
            StringArgumentType::GreedyPhrase,
            "'incorrect\"".to_owned()
        );
    }
}
