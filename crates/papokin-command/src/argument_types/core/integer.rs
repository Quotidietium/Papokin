use crate::{
    argument_types::{
        argument_type::{ArgumentType, JavaClientArgumentType},
        core::within_or_err,
    },
    errors::{command_syntax_error::CommandSyntaxError, error_types},
    string_reader::StringReader,
};

/// 表示解析 [`i32`] 的参数类型。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct IntegerArgumentType {
    pub min: i32,
    pub max: i32,
}

impl<S: crate::source::CommandSource> ArgumentType<S> for IntegerArgumentType {
    type Item = i32;

    fn parse(&self, reader: &mut StringReader) -> Result<i32, CommandSyntaxError> {
        let reader_start = reader.cursor();
        let result = reader.read_int()?;
        within_or_err(
            reader,
            reader_start,
            result,
            self.min,
            self.max,
            &error_types::INTEGER_TOO_LOW,
            &error_types::INTEGER_TOO_HIGH,
        )
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Integer {
            min: (self.min != i32::MIN).then_some(self.min),
            max: (self.max != i32::MAX).then_some(self.max),
        }
    }

    fn examples(&self) -> Vec<String> {
        examples!("0", "123", "-123")
    }
}

impl_copy_get!(IntegerArgumentType, i32);

impl IntegerArgumentType {
    /// 构造一个没有下界或上界的 [`IntegerArgumentType`]。
    #[must_use]
    pub const fn any() -> Self {
        Self {
            min: i32::MIN,
            max: i32::MAX,
        }
    }

    /// 构造一个*仅*具有指定下界的 [`IntegerArgumentType`]。
    #[must_use]
    pub const fn with_min(min: i32) -> Self {
        Self { min, max: i32::MAX }
    }

    /// 构造一个*仅*具有指定上界的 [`IntegerArgumentType`]。
    #[must_use]
    pub const fn with_max(max: i32) -> Self {
        Self { min: i32::MIN, max }
    }

    /// 构造一个具有给定界限的 [`IntegerArgumentType`]。
    #[must_use]
    pub const fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        argument_types::core::integer::IntegerArgumentType, errors::error_types,
        string_reader::StringReader,
    };

    #[test]
    fn parse_test() {
        let mut reader = StringReader::new("123");

        assert_parse_ok_reset!(&mut reader, IntegerArgumentType::any(), 123);

        assert_parse_ok_reset!(&mut reader, IntegerArgumentType::with_min(120), 123);
        assert_parse_err_reset!(
            &mut reader,
            IntegerArgumentType::with_min(130),
            &error_types::INTEGER_TOO_LOW
        );

        assert_parse_ok_reset!(&mut reader, IntegerArgumentType::with_max(200), 123);
        assert_parse_err_reset!(
            &mut reader,
            IntegerArgumentType::with_max(100),
            &error_types::INTEGER_TOO_HIGH
        );

        assert_parse_ok_reset!(&mut reader, IntegerArgumentType::new(100, 125), 123);
        assert_parse_err_reset!(
            &mut reader,
            IntegerArgumentType::new(100, 120),
            &error_types::INTEGER_TOO_HIGH
        );
        assert_parse_err_reset!(
            &mut reader,
            IntegerArgumentType::new(125, 150),
            &error_types::INTEGER_TOO_LOW
        );

        // 500_000_000 可放入 i32。
        reader = StringReader::new("500000000");
        assert_parse_ok_reset!(&mut reader, IntegerArgumentType::any(), 500_000_000);

        // 5_000_000_000 无法放入 i32。
        reader = StringReader::new("5000000000");
        assert_parse_err_reset!(
            &mut reader,
            IntegerArgumentType::any(),
            &error_types::READER_INVALID_INT
        );
    }
}
