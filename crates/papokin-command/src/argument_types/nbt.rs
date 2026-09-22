use papokin_nbt::tag::NbtTag;

use crate::{
    argument_types::argument_type::{ArgumentType, JavaClientArgumentType},
    context::command_context::CommandContext,
    errors::{command_syntax_error::CommandSyntaxError, error_types::CommandErrorType},
    snbt::SnbtParser,
    string_reader::StringReader,
    suggestion::suggestions::{Suggestions, SuggestionsBuilder},
};

use papokin_data::translation::java::ARGUMENT_NBT_EXPECTED_COMPOUND;
use papokin_nbt::compound::NbtCompound;

/// 从 SNBT 解析任意类型的 NBT 标签。
pub struct NbtTagArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for NbtTagArgumentType {
    type Item = NbtTag;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        SnbtParser::parse_for_commands(reader)
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::NbtTag
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        SnbtParser::parse_for_suggestions(builder)
    }

    fn examples(&self) -> Vec<String> {
        examples!("0", "0b", "0l", "0.0", "\"foo\"", "{foo=bar}", "[0]")
    }
}

impl NbtTagArgumentType {
    /// 从参数名称返回解析出的 [`NbtTag`]。
    pub fn get<'a, S: crate::source::CommandSource>(
        context: &'a CommandContext<S>,
        name: &'_ str,
    ) -> Result<&'a NbtTag, CommandSyntaxError> {
        context.get_argument(name)
    }
}

pub const EXPECTED_COMPOUND_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(ARGUMENT_NBT_EXPECTED_COMPOUND);

/// 从 SNBT 中**仅**解析复合 NBT 标签。
pub struct NbtCompoundArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for NbtCompoundArgumentType {
    type Item = NbtCompound;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        SnbtParser::parse_for_commands(reader).and_then(|tag| {
            if let NbtTag::Compound(compound) = tag {
                Ok(compound)
            } else {
                Err(EXPECTED_COMPOUND_ERROR_TYPE.create(reader))
            }
        })
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::NbtCompound
    }

    fn examples(&self) -> Vec<String> {
        examples!("{}", "{x: 3}")
    }
}

impl NbtCompoundArgumentType {
    /// 从参数名称返回解析出的 [`NbtCompound`]。
    pub fn get<'a, S: crate::source::CommandSource>(
        context: &'a CommandContext<S>,
        name: &'_ str,
    ) -> Result<&'a NbtCompound, CommandSyntaxError> {
        context.get_argument(name)
    }
}

pub use super::nbt_path::{NbtPath, NbtPathArgumentType, NbtPathNode};
