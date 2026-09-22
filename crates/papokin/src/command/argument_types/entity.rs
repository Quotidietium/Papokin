use crate::command::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::command::argument_types::entity_selector::EntitySelector;
use crate::command::argument_types::entity_selector::parser::{
    EntitySelectorParser, EntitySelectorParserSuggestions,
};
use crate::command::context::command_context::CommandContext;
use crate::command::context::command_source::CommandSource;
use crate::command::errors::command_syntax_error::CommandSyntaxError;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::string_reader::StringReader;
use crate::command::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use crate::entity::EntityBase;
use crate::entity::player::Player;
use papokin_data::translation;
use std::sync::Arc;

/// 一个 [`CommandErrorType`]，表示找不到任何实体。
pub const NO_ENTITIES_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_ENTITY_NOTFOUND_ENTITY);

/// 一个 [`CommandErrorType`]，表示找不到任何玩家。
pub const NO_PLAYERS_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_ENTITY_NOTFOUND_PLAYER);

/// 一个 [`CommandErrorType`]，表示实体选择器只允许玩家。
pub const ONLY_PLAYERS_ALLOWED_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_PLAYER_ENTITIES);

/// 一个 [`CommandErrorType`]，表示实体选择器只允许 1 个实体。
pub const NOT_SINGLE_ENTITY_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_ENTITY_TOOMANY);

/// 一个 [`CommandErrorType`]，表示实体选择器只允许 1 名玩家。
pub const NOT_SINGLE_PLAYER_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_PLAYER_TOOMANY);

pub const ENTITY_SELECTOR_PERMISSION: &str = "minecraft:command.selector";

/// 表示用于选择实体的参数类型。
///
/// 此参数类型包含以下变体：
/// - [`EntityArgumentType::Entity`]，用于单个实体。
/// - [`EntityArgumentType::Entities`]，用于任意数量的实体。
/// - [`EntityArgumentType::Player`]，用于单个玩家。
/// - [`EntityArgumentType::Players`]，用于任意数量的玩家。
///
/// 虽然此参数类型确实会解析 `EntitySelector`，但不应直接使用它。
/// 相反，请使用接受 [`CommandContext`] 的某个关联函数
/// 以及你的参数名称：
/// - [`EntityArgumentType::get_entity`]
/// - [`EntityArgumentType::get_entities`]
/// - [`EntityArgumentType::get_player`]
/// - [`EntityArgumentType::get_players`]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EntityArgumentType {
    Entity,
    Entities,
    Player,
    Players,
}

impl EntityArgumentType {
    const fn is_single(self) -> bool {
        matches!(self, Self::Entity | Self::Player)
    }

    const fn is_players_only(self) -> bool {
        matches!(self, Self::Player | Self::Players)
    }
}

impl ArgumentType<CommandSource> for EntityArgumentType {
    type Item = EntitySelector;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        self.parse_with_allow_selectors(reader, true)
    }

    fn parse_with_source(
        &self,
        reader: &mut StringReader,
        source: &CommandSource,
    ) -> Result<Self::Item, CommandSyntaxError> {
        self.parse_with_allow_selectors(reader, source.has_permission(ENTITY_SELECTOR_PERMISSION))
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Entity {
            flags: (self.is_single() as u8 * JavaClientArgumentType::ENTITY_FLAG_ONLY_SINGLE)
                | (self.is_players_only() as u8 * JavaClientArgumentType::ENTITY_FLAG_PLAYERS_ONLY),
        }
    }

    fn examples(&self) -> Vec<String> {
        examples!(
            "Herobrine",
            "98765",
            "@a",
            "@p[limit=2]",
            "@e[type=creeper]",
            "5e5677dc-bb96-4669-a4ab-60468b574e8e"
        )
    }

    fn list_suggestions(
        &self,
        context: &CommandContext,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        EntitySelectorParserSuggestions::list_suggestions(context, &builder)
    }
}

impl EntityArgumentType {
    fn parse_with_allow_selectors(
        self,
        reader: &mut StringReader,
        allow_selectors: bool,
    ) -> Result<<Self as ArgumentType<CommandSource>>::Item, CommandSyntaxError> {
        let selector = {
            let parser = EntitySelectorParser::new(reader, allow_selectors);
            parser.parse_and_consume()?
        };
        if selector.max_selected > 1 && self.is_single() {
            reader.set_cursor(0);
            Err(if self.is_players_only() {
                NOT_SINGLE_PLAYER_ERROR_TYPE.create(reader)
            } else {
                NOT_SINGLE_ENTITY_ERROR_TYPE.create(reader)
            })
        } else if selector.includes_entities
            && self.is_players_only()
            && !selector.is_current_entity
        {
            reader.set_cursor(0);
            Err(ONLY_PLAYERS_ALLOWED_ERROR_TYPE.create(reader))
        } else {
            Ok(selector)
        }
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取单个实体。
    pub fn get_entity(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Arc<dyn EntityBase>, CommandSyntaxError> {
        context
            .get_argument::<EntitySelector>(name)?
            .find_single_entity(context.source.as_ref())
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取至少 1 个实体。
    pub fn get_entities(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Vec<Arc<dyn EntityBase>>, CommandSyntaxError> {
        let entities = Self::get_optional_entities(context, name)?;
        if entities.is_empty() {
            Err(NO_ENTITIES_ERROR_TYPE.create_without_context())
        } else {
            Ok(entities)
        }
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取任意数量的实体。
    pub fn get_optional_entities(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Vec<Arc<dyn EntityBase>>, CommandSyntaxError> {
        context
            .get_argument::<EntitySelector>(name)?
            .find_entities(context.source.as_ref())
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取单个玩家。
    pub fn get_player(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Arc<Player>, CommandSyntaxError> {
        context
            .get_argument::<EntitySelector>(name)?
            .find_single_player(context.source.as_ref())
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取至少 1 个玩家。
    pub fn get_players(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Vec<Arc<Player>>, CommandSyntaxError> {
        let players = Self::get_optional_players(context, name)?;
        if players.is_empty() {
            Err(NO_PLAYERS_ERROR_TYPE.create_without_context())
        } else {
            Ok(players)
        }
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取任意数量的玩家。
    pub fn get_optional_players(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Vec<Arc<Player>>, CommandSyntaxError> {
        context
            .get_argument::<EntitySelector>(name)?
            .find_players(context.source.as_ref())
    }
}
