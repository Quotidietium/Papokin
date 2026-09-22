use papokin_util::math::bounds::{DoubleBounds, IntBounds};

use crate::{
    argument_types::{
        FromStringReader,
        argument_type::{ArgumentType, JavaClientArgumentType},
    },
    errors::command_syntax_error::CommandSyntaxError,
    string_reader::StringReader,
};

/// 解析可按以下方式表示的 `i32` 闭区间范围：
/// - `value`：仅包含整数 `value`。
/// - `min..`：所有大于或等于 `min` 的整数。
/// - `..max`：所有小于等于 `max` 的整数。
/// - `min..max`：所有介于 `min` 和 `max` 之间（含两端）的整数。数学上可表示为 `[min, max]`。
pub struct IntRangeArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for IntRangeArgumentType {
    type Item = IntBounds;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        IntBounds::from_reader(reader)
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::IntRange
    }

    fn examples(&self) -> Vec<String> {
        examples!("8", "2..", "..-3", "-4..5")
    }
}

impl_copy_get!(IntRangeArgumentType, IntBounds);

/// 解析可按以下方式表示的 `f64` 闭区间范围：
/// - `value`：仅包含数值 `value`。
/// - `min..`：所有大于或等于 `min` 的数。
/// - `..max`：所有小于等于 `max` 的数值。
/// - `min..max`：所有介于 `min` 和 `max` 之间（含两端）的数。数学上可表示为 `[min, max]`。
pub struct FloatRangeArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for FloatRangeArgumentType {
    type Item = DoubleBounds;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        DoubleBounds::from_reader(reader)
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::FloatRange
    }

    fn examples(&self) -> Vec<String> {
        examples!("8", "8.0", "5..", "..-3.8", "-4.2..5", "0.1..0.2")
    }
}

impl_copy_get!(FloatRangeArgumentType, DoubleBounds);

#[cfg(test)]
mod test {
    use papokin_util::math::bounds::{DoubleBounds, IntBounds};

    use crate::{
        argument_types::{
            SWAPPED_BOUNDS_ERROR_TYPE,
            range::{FloatRangeArgumentType, IntRangeArgumentType},
        },
        string_reader::StringReader,
    };

    #[test]
    fn parse_int_ranges() {
        let mut reader = StringReader::new("-7");
        assert_parse_ok_reset!(&mut reader, IntRangeArgumentType, IntBounds::new(-7, -7));

        let mut reader = StringReader::new("3..");
        assert_parse_ok_reset!(
            &mut reader,
            IntRangeArgumentType,
            IntBounds::new_at_least(3)
        );

        let mut reader = StringReader::new("..4");
        assert_parse_ok_reset!(&mut reader, IntRangeArgumentType, IntBounds::new_at_most(4));

        let mut reader = StringReader::new("3..4");
        assert_parse_ok_reset!(&mut reader, IntRangeArgumentType, IntBounds::new(3, 4));

        let mut reader = StringReader::new("-0..0");
        assert_parse_ok_reset!(&mut reader, IntRangeArgumentType, IntBounds::new(0, 0));

        let mut reader = StringReader::new("2..1");
        assert_parse_err_reset!(
            &mut reader,
            IntRangeArgumentType,
            &SWAPPED_BOUNDS_ERROR_TYPE
        );
    }

    #[test]
    fn parse_float_ranges() {
        let mut reader = StringReader::new("-1");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(-1.0, -1.0)
        );

        let mut reader = StringReader::new("-1.0");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(-1.0, -1.0)
        );

        let mut reader = StringReader::new("0.");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(0.0, 0.0)
        );

        let mut reader = StringReader::new("3..");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new_at_least(3.0)
        );

        let mut reader = StringReader::new("..4");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new_at_most(4.0)
        );

        let mut reader = StringReader::new("3..4");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(3.0, 4.0)
        );

        let mut reader = StringReader::new("0.9");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(0.9, 0.9)
        );

        let mut reader = StringReader::new("0..9");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(0.0, 9.0)
        );

        let mut reader = StringReader::new("0...9");
        assert_parse_ok_reset!(
            &mut reader,
            FloatRangeArgumentType,
            DoubleBounds::new(0.0, 0.9)
        );

        let mut reader = StringReader::new("2..1");
        assert_parse_err_reset!(
            &mut reader,
            FloatRangeArgumentType,
            &SWAPPED_BOUNDS_ERROR_TYPE
        );
    }
}
