use papokin_data::translation;
use papokin_util::math::position::{BlockPos, ColumnPos};
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;

use crate::suggestion::suggestions::TextCoordinates;
use crate::{
    argument_types::{
        argument_type::{ArgumentType, JavaClientArgumentType},
        coordinates::{Coordinates, WorldCoordinate},
    },
    context::command_context::CommandContext,
    errors::{command_syntax_error::CommandSyntaxError, error_types::CommandErrorType},
    string_reader::StringReader,
    suggestion::suggestions::{Suggestions, SuggestionsBuilder},
};

pub const INCOMPLETE_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_POS2D_INCOMPLETE);

pub struct ColumnPosArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for ColumnPosArgumentType {
    type Item = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        if !reader.can_read_char() {
            return Err(INCOMPLETE_ERROR_TYPE.create(reader));
        }

        let start = reader.cursor();
        let x = WorldCoordinate::parse_integer(reader)?;
        if reader.peek() == Some(' ') {
            reader.skip();
            let z = WorldCoordinate::parse_integer(reader)?;
            Ok(Coordinates::World(Vector3 {
                x,
                y: WorldCoordinate::Relative(0.0),
                z,
            }))
        } else {
            reader.set_cursor(start);
            Err(INCOMPLETE_ERROR_TYPE.create(reader))
        }
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        let remainder = builder.remaining();

        let suggested_coordinates = if remainder.bytes().next() == Some(b'^') {
            TextCoordinates::Local
        } else {
            TextCoordinates::Global
        };

        builder.suggest_2d_coordinates(suggested_coordinates, |value| {
            ArgumentType::<S>::parse(self, &mut StringReader::new(value)).is_ok()
        })
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::ColumnPos
    }

    fn examples(&self) -> Vec<String> {
        examples!("0 0", "~ ~", "~-1 ~2")
    }
}

impl ColumnPosArgumentType {
    ///以 [`ColumnPos`] 的形式返回 [`CommandContext`] 中已解析的坐标参数。
    ///
    /// # Arguments
    /// * `context` - 包含以给定参数名解析出的 `Coordinates` 的 [`CommandContext`]。
    /// * `name` - 已解析参数的名称。
    ///
    /// # Returns
    /// 包含解析出的参数所表示位置的 `ColumnPos`，包装在 `Ok` 中，
    /// 或在无法解析时返回带有相应 [`CommandSyntaxError`] 的 `Err`。
    pub fn get_column_pos<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<ColumnPos, CommandSyntaxError> {
        let block_pos = BlockPos::floored_v(
            context
                .get_argument::<Coordinates>(name)?
                .resolve(context.source.as_ref()),
        );
        Ok(ColumnPos(Vector2::new(block_pos.0.x, block_pos.0.z)))
    }
}
