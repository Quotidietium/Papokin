use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::argument_types::coordinates::Coordinates;
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::CommandErrorType;
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder, TextCoordinates};
use papokin_data::translation;
use papokin_util::math::position::BlockPos;

pub const NOT_LOADED_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_POS_UNLOADED);
pub const OUT_OF_WORLD_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_POS_OUTOFWORLD);
pub const OUT_OF_BOUNDS_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_POS_OUTOFBOUNDS);

/// 表示方块位置的三维向量所用的参数类型。
///
/// 解析得到的 [`Coordinates`] 可通过以下方法之一转换为 [`BlockPos`]
/// 以下关联方法：
/// - [`BlockPosArgumentType::get_block_pos`]：普通转换。
/// - [`BlockPosArgumentType::get_loaded_block_pos`]：将坐标转换为*已加载*的 `BlockPos`。
///   大多数情况下你都应该使用它（如果你需要以某种方式更新已加载的位置）。
/// - [`BlockPosArgumentType::get_loaded_block_pos_in_world`]：将坐标转换为*已加载*的 `BlockPos`
///   在给定的世界中。
/// - [`BlockPosArgumentType::get_spawnable_pos`]：将坐标转换为 `BlockPos`，其中
///   玩家可以生成的位置。
pub struct BlockPosArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for BlockPosArgumentType {
    type Item = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        if reader.peek() == Some('^') {
            Coordinates::parse_local(reader)
        } else {
            Coordinates::parse_world_integers(reader)
        }
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::BlockPos
    }

    fn examples(&self) -> Vec<String> {
        examples!("1 3 5", "-3 ~24 ~-1", "80 80 80", "^ ^9 ^56")
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

        builder.suggest_3d_coordinates(suggested_coordinates, |value| {
            ArgumentType::<S>::parse(self, &mut StringReader::new(value)).is_ok()
        })
    }
}

impl BlockPosArgumentType {
    ///以 [`BlockPos`] 的形式返回 [`CommandContext`] 中已解析的坐标参数。
    ///
    /// # Arguments
    /// * `context` - 包含以给定参数名解析出的 `Coordinates` 的 [`CommandContext`]。
    /// * `name` - 已解析参数的名称。
    ///
    /// # Returns
    /// 已加载的 `BlockPos`，包含解析参数所表示的位置，包裹在 `Ok` 中，
    /// 或在无法解析时返回带有相应 [`CommandSyntaxError`] 的 `Err`。
    pub fn get_block_pos<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<BlockPos, CommandSyntaxError> {
        Ok(BlockPos::floored_v(
            context
                .get_argument::<Coordinates>(name)?
                .resolve(context.source.as_ref()),
        ))
    }

    ///以已加载的 [`BlockPos`] 形式返回 [`CommandContext`] 中已解析的坐标参数。
    ///
    /// # Arguments
    /// * `context` - 包含以给定参数名解析出的 `Coordinates` 的 [`CommandContext`]。
    ///   其 `CommandSource` 还决定在哪个世界中检查加载状态。
    /// * `name` - 已解析参数的名称。
    ///
    /// # Returns
    /// 已加载的 `BlockPos`，包含解析参数所表示的位置，包裹在 `Ok` 中，
    /// 或在无法解析时返回带有相应 [`CommandSyntaxError`] 的 `Err`。
    pub fn get_loaded_block_pos<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<BlockPos, CommandSyntaxError> {
        let pos = Self::get_block_pos(context, name)?;
        context.source.check_block_loaded(&pos)?;
        Ok(pos)
    }

    ///以 [`BlockPos`] 的形式返回 [`CommandContext`] 中已解析的坐标参数
    /// 玩家可以生成的地方。
    ///
    /// # Arguments
    /// * `context` - 包含以给定参数名解析出的 `Coordinates` 的 [`CommandContext`]。
    /// * `name` - 已解析参数的名称。
    ///
    /// # Returns
    /// 已加载的 `BlockPos`，包含解析参数所表示的位置，包裹在 `Ok` 中，
    /// 或在无法解析时返回带有相应 [`CommandSyntaxError`] 的 `Err`。
    pub fn get_spawnable_pos<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<BlockPos, CommandSyntaxError> {
        let pos = Self::get_block_pos(context, name)?;
        if is_valid_block_pos(pos) {
            Ok(pos)
        } else {
            Err(OUT_OF_BOUNDS_ERROR_TYPE.create_without_context())
        }
    }
}

#[must_use]
pub fn is_valid_block_pos(dest: BlockPos) -> bool {
    (-30_000_000..30_000_000).contains(&dest.0.x)
        && (-30_000_000..30_000_000).contains(&dest.0.z)
        && (-20_000_000..20_000_000).contains(&dest.0.y)
}
