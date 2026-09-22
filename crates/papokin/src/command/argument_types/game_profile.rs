use crate::command::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::command::argument_types::entity::ONLY_PLAYERS_ALLOWED_ERROR_TYPE;
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
use crate::net::authentication::lookup_profile_by_name_blocking;
use crate::net::{GameProfile, offline_uuid};
use crate::server::Server;
use arc_swap::ArcSwap;
use papokin_data::translation;
use std::sync::Arc;
use uuid::Uuid;

pub const UNKNOWN_PLAYER_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_PLAYER_UNKNOWN);

/// [`GameProfileArgumentType`] 的一个结果，可被解析为
/// 一个或多个 [`GameProfile`]，无论成功与否。
pub enum GameProfileResult {
    Selector(Box<EntitySelector>),
    Name(String),
    Uuid(Uuid),
}

impl GameProfileResult {
    fn unknown_player_syntax_error() -> CommandSyntaxError {
        UNKNOWN_PLAYER_ERROR_TYPE.create_without_context()
    }

    /// 借助 [`CommandSource`] 解析此结果。
    ///
    /// # Warning
    ///
    /// 不要对其中一个数据锁的写/读访问加锁
    /// 之前调用此方法，因为这可能导致*死锁*：
    /// - `server.data.user_cache`
    /// - `server.data.operator_config`
    /// - `server.data.banned_player_list`
    /// - `server.data.whitelist_config`
    ///
    /// 相反，应在对锁调用 `write()`/`read()` *之前*调用此方法。
    pub fn resolve(&self, source: &CommandSource) -> Result<Vec<GameProfile>, CommandSyntaxError> {
        match self {
            Self::Selector(selector) => {
                let players = selector.find_players(source)?;
                if players.is_empty() {
                    return Err(
                        crate::command::argument_types::entity::NO_PLAYERS_ERROR_TYPE
                            .create_without_context(),
                    );
                }
                Ok(players.iter().map(|p| p.gameprofile.clone()).collect())
            }
            Self::Name(name) => {
                let server = source.server();
                if let Some(player) = server.get_player_by_name(name) {
                    return Ok(vec![player.gameprofile.clone()]);
                }

                let cached_entry = server
                    .data
                    .user_cache
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get_by_name(name);
                if let Some(entry) = cached_entry {
                    return Ok(vec![Self::profile_from_uuid_name(entry.uuid, entry.name)]);
                }

                if let Some(profile) = Self::resolve_known_profile_by_name(server, name) {
                    return Ok(vec![profile]);
                }

                if server.advanced_config.networking.java.online_mode {
                    match lookup_profile_by_name_blocking(
                        name,
                        &server.advanced_config.networking.java.authentication,
                        Some(server),
                    ) {
                        Ok(Some((uuid, resolved_name))) => {
                            server
                                .data
                                .user_cache
                                .write()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .upsert(uuid, resolved_name.clone());
                            Ok(vec![Self::profile_from_uuid_name(uuid, resolved_name)])
                        }
                        _ => Err(Self::unknown_player_syntax_error()),
                    }
                } else if let Ok(uuid) = offline_uuid(name) {
                    let profile = Self::profile_from_uuid_name(uuid, name.clone());
                    server
                        .data
                        .user_cache
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .upsert(profile.id, profile.name.clone());
                    Ok(vec![profile])
                } else {
                    Err(Self::unknown_player_syntax_error())
                }
            }
            Self::Uuid(uuid) => {
                let server = source.server();

                if let Some(player) = server.get_player_by_uuid(*uuid) {
                    return Ok(vec![player.gameprofile.clone()]);
                }

                let cached_entry = server
                    .data
                    .user_cache
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get_by_uuid(*uuid);
                if let Some(entry) = cached_entry {
                    return Ok(vec![Self::profile_from_uuid_name(entry.uuid, entry.name)]);
                }

                if let Some(profile) = Self::resolve_known_profile_by_uuid(server, *uuid) {
                    return Ok(vec![profile]);
                }

                Err(Self::unknown_player_syntax_error())
            }
        }
    }

    fn resolve_known_profile_by_name(server: &Server, name: &str) -> Option<GameProfile> {
        let ops = server
            .data
            .operator_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(op) = ops.ops.iter().find(|op| op.name.eq_ignore_ascii_case(name)) {
            return Some(Self::profile_from_uuid_name(op.uuid, op.name.clone()));
        }

        let banned_players = server
            .data
            .banned_player_list
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = banned_players
            .banned_players
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        {
            return Some(Self::profile_from_uuid_name(entry.uuid, entry.name.clone()));
        }

        let whitelist = server
            .data
            .whitelist_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = whitelist
            .whitelist
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        {
            return Some(Self::profile_from_uuid_name(entry.uuid, entry.name.clone()));
        }

        None
    }

    fn resolve_known_profile_by_uuid(server: &Server, uuid: Uuid) -> Option<GameProfile> {
        let ops = server
            .data
            .operator_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(op) = ops.ops.iter().find(|op| op.uuid == uuid) {
            return Some(Self::profile_from_uuid_name(op.uuid, op.name.clone()));
        }

        let banned_players = server
            .data
            .banned_player_list
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = banned_players
            .banned_players
            .iter()
            .find(|entry| entry.uuid == uuid)
        {
            return Some(Self::profile_from_uuid_name(entry.uuid, entry.name.clone()));
        }

        let whitelist = server
            .data
            .whitelist_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = whitelist.whitelist.iter().find(|entry| entry.uuid == uuid) {
            return Some(Self::profile_from_uuid_name(entry.uuid, entry.name.clone()));
        }

        None
    }

    #[allow(clippy::missing_const_for_fn)]
    fn profile_from_uuid_name(uuid: Uuid, name: String) -> GameProfile {
        GameProfile {
            id: uuid,
            name,
            properties: ArcSwap::new(Arc::new(vec![])),
            profile_actions: None,
        }
    }
}

/// 一种参数类型，用于解析一个或多个 [`GameProfile`]。
///
/// 使用 [`GameProfileArgumentType::get`] 可自动获取一个 `Vec` 的
/// 为某个参数解析 [`GameProfile`]，需提供 [`CommandContext`] 与
/// 参数名。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GameProfileArgumentType;

impl ArgumentType<CommandSource> for GameProfileArgumentType {
    type Item = GameProfileResult;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        Self::parse_with_allow_selectors(reader, true)
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::GameProfile
    }

    fn list_suggestions(
        &self,
        context: &CommandContext,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        EntitySelectorParserSuggestions::list_suggestions(context, &builder)
    }

    fn examples(&self) -> Vec<String> {
        examples!("Herobrine", "98765", "@a", "@p[limit=2]")
    }
}

impl GameProfileArgumentType {
    fn parse_with_allow_selectors(
        reader: &mut StringReader,
        allow_selectors: bool,
    ) -> Result<<Self as ArgumentType<CommandSource>>::Item, CommandSyntaxError> {
        if reader.peek() == Some('@') {
            // 我们读取一个选择器变量。
            let parser = EntitySelectorParser::new(reader, allow_selectors);
            let selector = parser.parse_and_consume()?;
            if selector.includes_entities {
                Err(ONLY_PLAYERS_ALLOWED_ERROR_TYPE.create(reader))
            } else {
                Ok(GameProfileResult::Selector(Box::new(selector)))
            }
        } else {
            // 我们读取 UUID 或玩家名。
            let i = reader.cursor();
            while reader.can_read_char() && reader.peek() != Some(' ') {
                reader.skip();
            }
            let string = &reader.string()[i..reader.cursor()];
            Ok(Uuid::try_parse(string).map_or_else(
                |_| GameProfileResult::Name(string.to_owned()),
                GameProfileResult::Uuid,
            ))
        }
    }

    /// 尝试从所提供 [`CommandContext`] 的已解析参数中获取任意数量的 [`GameProfile`]。
    ///
    /// # Warning
    ///
    /// 不要对其中一个数据锁的写/读访问加锁
    /// 之前调用此方法，因为这可能导致*死锁*：
    /// - `server.data.user_cache`
    /// - `server.data.operator_config`
    /// - `server.data.banned_player_list`
    /// - `server.data.whitelist_config`
    ///
    /// 相反，应在对锁调用 `write()`/`read()` *之前*调用此函数。
    pub fn get(
        context: &CommandContext<'_>,
        name: &str,
    ) -> Result<Vec<GameProfile>, CommandSyntaxError> {
        context
            .get_argument::<GameProfileResult>(name)?
            .resolve(context.source.as_ref())
    }
}
