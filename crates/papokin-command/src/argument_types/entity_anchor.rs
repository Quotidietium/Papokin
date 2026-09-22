use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::CommandErrorType;
use crate::source::CommandSource;
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_data::translation;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;

pub const INVALID_ERROR_TYPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::ARGUMENT_ANCHOR_INVALID);

pub struct EntityAnchorArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for EntityAnchorArgumentType {
    type Item = EntityAnchor;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let i = reader.cursor();
        let anchor = reader.read_unquoted_string();
        EntityAnchor::from_id(anchor.as_str()).map_or_else(
            || {
                reader.set_cursor(i);
                Err(INVALID_ERROR_TYPE.create(reader, TextComponent::text(anchor)))
            },
            Ok,
        )
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::EntityAnchor
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        builder.filter_and_suggest(&["eyes", "feet"]).build()
    }

    fn examples(&self) -> Vec<String> {
        examples!("eyes", "feet")
    }
}

impl_copy_get!(EntityAnchorArgumentType, EntityAnchor);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum EntityAnchor {
    #[default]
    Feet,
    Eyes,
}

impl EntityAnchor {
    /// 获取标识为所提供 ID 的 [`EntityAnchor`]。
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "feet" => Some(Self::Feet),
            "eyes" => Some(Self::Eyes),
            _ => None,
        }
    }

    /// 获取此 [`EntityAnchor`] 的 ID
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Feet => "feet",
            Self::Eyes => "eyes",
        }
    }

    /// 获取来源相对于此锚点的位置。
    #[must_use]
    pub fn position_at_source(self, command_source: &impl CommandSource) -> Vector3<f64> {
        command_source.anchor_position(self)
    }
}
