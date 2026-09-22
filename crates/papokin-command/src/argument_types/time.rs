use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::CommandErrorType;
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_data::translation;
use papokin_util::text::TextComponent;

pub const INVALID_UNIT_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_TIME_INVALID_UNIT);
pub const TICK_COUNT_TOO_LOW_ERROR_TYPE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_TIME_TICK_COUNT_TOO_LOW);

/// 表示解析时间值的参数类型，该值可以
/// 具有以下后缀之一：
/// - `t` 或 *无后缀*：刻
/// - `s`：现实秒数（20 刻）
/// - `d`：Minecraft 天（24,000 刻）
///
/// 此参数类型所提供的 `i32` 以*刻*为单位表示数值。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TimeArgumentType {
    /// 此类型接受的最短时间（以刻为单位）。
    pub min: i32,
}

impl TimeArgumentType {
    ///返回一个接受任意时间值的 [`TimeArgumentType`]。
    #[must_use]
    pub const fn any() -> Self {
        Self::new(0)
    }

    ///返回一个 [`TimeArgumentType`]，它接受任何持续至少 `min` 刻的时间值。
    #[must_use]
    pub const fn new(min: i32) -> Self {
        Self { min }
    }
}

impl<S: crate::source::CommandSource> ArgumentType<S> for TimeArgumentType {
    type Item = i32;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let value = reader.read_float()?;
        let unit = reader.read_unquoted_string();
        // 找到本单位的刻换算关系
        let ticks_per_unit = match unit.as_str() {
            "t" | "" => 1,
            "s" => 20,
            "d" => 24000,
            _ => return Err(INVALID_UNIT_ERROR_TYPE.create(reader)),
        };
        let ticks = (ticks_per_unit as f32 * value).round() as i32;
        if ticks < self.min {
            Err(TICK_COUNT_TOO_LOW_ERROR_TYPE.create(
                reader,
                TextComponent::text(self.min.to_string()),
                TextComponent::text(ticks.to_string()),
            ))
        } else {
            Ok(ticks)
        }
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Time { min: self.min }
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        suggestions_builder: SuggestionsBuilder,
    ) -> Suggestions {
        let mut reader = StringReader::new(suggestions_builder.remaining());
        if reader.read_float().is_err() {
            suggestions_builder.build()
        } else {
            suggestions_builder
                .filter_and_suggest(&["t", "s", "d"])
                .build()
        }
    }

    fn examples(&self) -> Vec<String> {
        examples!("12d", "14s", "789", "450t")
    }
}

impl TimeArgumentType {
    ///以整数形式返回 [`CommandContext`] 中已解析的时间参数
    /// 其持续时间（以刻为单位）。
    pub fn get<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<i32, CommandSyntaxError> {
        Ok(*context.get_argument(name)?)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        argument_types::{time, time::TimeArgumentType},
        string_reader::StringReader,
    };

    #[test]
    fn parse_ticks() {
        // 无单位
        let mut reader = StringReader::new("15");

        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 15);
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::new(10), 15);
        assert_parse_err_reset!(
            &mut reader,
            TimeArgumentType::new(20),
            &time::TICK_COUNT_TOO_LOW_ERROR_TYPE
        );

        // 单位
        let mut reader = StringReader::new("95t");

        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 95);
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::new(50), 95);
        assert_parse_err_reset!(
            &mut reader,
            TimeArgumentType::new(150),
            &time::TICK_COUNT_TOO_LOW_ERROR_TYPE
        );
    }

    #[test]
    fn parse_other_units() {
        // 秒
        let mut reader = StringReader::new("6s");
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 120);

        // 该参数类型会在相乘后对小数舍入
        // 到最接近的整数。
        let mut reader = StringReader::new("4.5s");
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 90);
        let mut reader = StringReader::new("4.633s");
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 93);

        let mut reader = StringReader::new("-0.2s");
        assert_parse_err_reset!(
            &mut reader,
            TimeArgumentType::any(),
            &time::TICK_COUNT_TOO_LOW_ERROR_TYPE
        );

        // Minecraft 天数
        let mut reader = StringReader::new("9d");
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 24000 * 9);
        let mut reader = StringReader::new("7.1234d");
        assert_parse_ok_reset!(&mut reader, TimeArgumentType::any(), 170962);

        // 无效的单位
        let mut reader = StringReader::new("14m");
        assert_parse_err_reset!(
            &mut reader,
            TimeArgumentType::any(),
            &time::INVALID_UNIT_ERROR_TYPE
        );
        let mut reader = StringReader::new("1w");
        assert_parse_err_reset!(
            &mut reader,
            TimeArgumentType::any(),
            &time::INVALID_UNIT_ERROR_TYPE
        );
    }
}
