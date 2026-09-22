use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::argument_types::coordinates::{Coordinates, MIXED_TYPE_ERROR_TYPE};
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::CommandErrorType;
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder, TextCoordinates};
use papokin_data::translation;
use papokin_util::math::vector3::Vector3;

pub const INCOMPLETE_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_POS3D_INCOMPLETE);
pub const ERROR_NOT_COMPLETE: CommandErrorType<0> = INCOMPLETE_ERROR_TYPE;
pub const ERROR_MIXED_TYPE: CommandErrorType<0> = MIXED_TYPE_ERROR_TYPE;

#[derive(Debug, Default, Clone, Copy)]
/// 三维向量所用的参数类型。
pub enum Vec3ArgumentType {
    /// 默认的 `Vec3ArgumentType` 变体。
    ///
    /// 用于表示世界中的某个位置，
    /// 你几乎总是都会想用这个。
    ///
    /// 对于每个坐标，若未使用小数（`.`）符号，
    /// (坐标为整数) 且不是相对坐标，
    /// 则会为其加上 `+0.5` 的偏移量。
    ///
    #[default]
    Default,
    /// 此 `Vec3ArgumentType` 变体不会进行中心校正。
    Uncorrected,
}

impl Vec3ArgumentType {
    /// 返回此参数类型是否以整数为中心。
    #[must_use]
    pub const fn centers_integers(&self) -> bool {
        matches!(self, Self::Default)
    }

    /// 使用给定的中心校正创建新的 `Vec3ArgumentType`。
    #[must_use]
    pub const fn new(centers_integers: bool) -> Self {
        if centers_integers {
            Self::Default
        } else {
            Self::Uncorrected
        }
    }
}

impl<S: crate::source::CommandSource> ArgumentType<S> for Vec3ArgumentType {
    type Item = Coordinates;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        if reader.peek() == Some('^') {
            Coordinates::parse_local(reader)
        } else {
            Coordinates::parse_world(reader, self.centers_integers())
        }
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Vec3
    }

    fn examples(&self) -> Vec<String> {
        vec![
            "0 0 0".to_string(),
            "~ ~ ~".to_string(),
            "^ ^ ^".to_string(),
            "^1 ^ ^-5".to_string(),
            "0.1 -0.5 .9".to_string(),
            "~0.5 ~1 ~-5".to_string(),
        ]
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        let remainder = builder.remaining();

        let suggestioned_coordinates = if remainder.bytes().next() == Some(b'^') {
            TextCoordinates::Local
        } else {
            TextCoordinates::Global
        };

        builder.suggest_3d_coordinates(suggestioned_coordinates, |value| {
            ArgumentType::<S>::parse(self, &mut StringReader::new(value)).is_ok()
        })
    }
}

impl Vec3ArgumentType {
    ///将 [`CommandContext`] 中已解析的三维向量作为一组 [`Coordinates`] 返回。
    pub fn get_coordinates<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<Coordinates, CommandSyntaxError> {
        Ok(*context.get_argument(name)?)
    }

    ///返回 [`CommandContext`] 中已解析的三维向量，并将其换算为 [`Vector3`]。
    pub fn get_vector3<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<Vector3<f64>, CommandSyntaxError> {
        Ok(Self::get_coordinates(context, name)?.resolve(context.source.as_ref()))
    }
}
