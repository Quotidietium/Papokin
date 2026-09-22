use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::argument_types::coordinates::{Coordinates, WorldCoordinate};
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::CommandErrorType;
use crate::string_reader::StringReader;
use papokin_data::translation;
use papokin_util::math::vector3::Vector3;

pub const NOT_COMPLETE_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_ROTATION_INCOMPLETE);

pub struct RotationArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for RotationArgumentType {
    type Item = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let i = reader.cursor();
        if reader.can_read_char() {
            let y = WorldCoordinate::parse(reader, false)?;
            if reader.peek() == Some(' ') {
                reader.skip();
                let x = WorldCoordinate::parse(reader, false)?;
                Ok(Coordinates::World(Vector3::new(
                    x,
                    y,
                    WorldCoordinate::Relative(0.0),
                )))
            } else {
                reader.set_cursor(i);
                Err(Self::syntax_error(reader))
            }
        } else {
            Err(Self::syntax_error(reader))
        }
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Rotation
    }

    fn examples(&self) -> Vec<String> {
        examples!("1 1", "~5 4", "~-6 ~-6")
    }
}

impl RotationArgumentType {
    fn syntax_error(reader: &StringReader) -> CommandSyntaxError {
        NOT_COMPLETE_ERROR_TYPE.create(reader)
    }

    ///返回以 [`Coordinates`] 形式表示的、由已解析旋转参数得到的旋转量。
    ///
    /// 使用 [`Coordinates::rotation`] 将坐标解析为旋转 `Vector22`。
    ///
    /// 如果旋转已成功通过 `Ok` 提供：
    /// - 返回坐标的 *x* 坐标是绝对/相对**俯仰角**。
    /// - 返回坐标的 *y* 坐标是绝对/相对**偏航角**。
    /// - 返回坐标的 *z* 坐标始终为 0。
    pub fn get<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<Coordinates, CommandSyntaxError> {
        context.get_argument::<Coordinates>(name).copied()
    }
}
