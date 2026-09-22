use std::sync::atomic::Ordering;

use papokin_config::whitelist::WhitelistEntry;
use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::game_profile::GameProfileArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::command::suggestion::provider::{SuggestionProvider, SuggestionProviderResult};
use crate::command::suggestion::suggestions::SuggestionsBuilder;
use crate::data::{LoadJSONConfiguration, SaveJSONConfiguration, whitelist::WhitelistConfig};
use crate::server::Server;

const DESCRIPTION: &str = "管理服务器白名单。";
const PERMISSION: &str = "minecraft:command.whitelist";

const ERROR_ALREADY_ON: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_WHITELIST_ALREADYON);

const ERROR_ALREADY_OFF: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_WHITELIST_ALREADYOFF);

const ERROR_ADD_FAILED: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_WHITELIST_ADD_FAILED);

const ERROR_REMOVE_FAILED: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_WHITELIST_REMOVE_FAILED);

pub fn kick_non_whitelisted_players(server: &Server) {
    let whitelist = server.data.whitelist_config.read().unwrap();
    let ops = server.data.operator_config.read().unwrap();
    if server.basic_config.enforce_whitelist && server.white_list.load(Ordering::Relaxed) {
        for player in server.get_all_players() {
            if ops.get_entry(&player.gameprofile.id).is_some()
                || whitelist.is_whitelisted(&player.gameprofile)
            {
                continue;
            }
            player.kick(&papokin_macros::translate!(
                translation::java::MULTIPLAYER_DISCONNECT_NOT_WHITELISTED
            ));
        }
    }
}

struct OnExecutor;

impl CommandExecutor for OnExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server = context.source.server();
        let previous = server.white_list.swap(true, Ordering::Relaxed);
        if previous {
            Err(ERROR_ALREADY_ON.create_without_context())
        } else {
            let mut event =
                crate::plugin::api::events::server::whitelist_toggle::WhitelistToggleEvent::new(
                    true,
                );
            server.plugin_manager.fire_blocking(server, &mut event);
            context.source.send_feedback(
                papokin_macros::translate!(translation::java::COMMANDS_WHITELIST_ENABLED),
                true,
            );
            kick_non_whitelisted_players(server);
            Ok(1)
        }
    }
}

struct OffExecutor;

impl CommandExecutor for OffExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server = context.source.server();
        let previous = server.white_list.swap(false, Ordering::Relaxed);
        if previous {
            let mut event =
                crate::plugin::api::events::server::whitelist_toggle::WhitelistToggleEvent::new(
                    false,
                );
            server.plugin_manager.fire_blocking(server, &mut event);
            context.source.send_feedback(
                papokin_macros::translate!(translation::java::COMMANDS_WHITELIST_DISABLED),
                true,
            );
            Ok(1)
        } else {
            Err(ERROR_ALREADY_OFF.create_without_context())
        }
    }
}

struct ListExecutor;

impl CommandExecutor for ListExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server = context.source.server();
        let whitelist_guard = server.data.whitelist_config.read().unwrap();
        let whitelist = &whitelist_guard.whitelist;
        if whitelist.is_empty() {
            context.source.send_feedback(
                TextComponent::translate(translation::java::COMMANDS_WHITELIST_NONE, []),
                false,
            );
            return Ok(0);
        }

        let names = whitelist
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        let count = whitelist.len() as i32;

        context.source.send_feedback(
            TextComponent::translate(
                translation::java::COMMANDS_WHITELIST_LIST,
                [
                    TextComponent::text(count.to_string()),
                    TextComponent::text(names),
                ],
            ),
            false,
        );

        Ok(count)
    }
}

struct ReloadExecutor;

impl CommandExecutor for ReloadExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server = context.source.server();
        *server.data.whitelist_config.write().unwrap() = WhitelistConfig::load();
        context.source.send_feedback(
            papokin_macros::translate!(translation::java::COMMANDS_WHITELIST_RELOADED),
            true,
        );
        kick_non_whitelisted_players(server);
        Ok(1)
    }
}

struct AddSuggestionProvider;

impl SuggestionProvider for AddSuggestionProvider {
    fn suggest(
        &self,
        context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        let whitelist = context.server().data.whitelist_config.read().unwrap();
        for player in context.server().get_all_players() {
            if !whitelist.is_whitelisted(&player.gameprofile) {
                builder = builder.suggest(player.gameprofile.name.clone());
            }
        }
        builder.build()
    }
}

struct AddExecutor;

impl CommandExecutor for AddExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = GameProfileArgumentType::get(context, "targets")?;
        let server = context.source.server();
        let mut whitelist = server.data.whitelist_config.write().unwrap();
        let mut successes: i32 = 0;
        let mut modified = false;
        let mut added: Vec<(String, uuid::Uuid)> = Vec::new();

        for profile in &targets {
            if let Some(existing_entry) = whitelist
                .whitelist
                .iter_mut()
                .find(|entry| entry.uuid == profile.id)
            {
                if existing_entry.name != profile.name {
                    existing_entry.name.clone_from(&profile.name);
                    modified = true;
                }
                continue;
            }
            whitelist
                .whitelist
                .push(WhitelistEntry::new(profile.id, profile.name.clone()));
            context.source.send_feedback(
                papokin_macros::translate!(
                    translation::java::COMMANDS_WHITELIST_ADD_SUCCESS,
                    TextComponent::text(profile.name.clone())
                ),
                true,
            );
            added.push((profile.name.clone(), profile.id));
            successes += 1;
            modified = true;
        }

        if modified {
            whitelist.save();
        }
        drop(whitelist);

        for (name, uuid) in added {
            let mut event =
                crate::plugin::api::events::server::whitelist_state_update::WhitelistStateUpdateEvent::new(
                    name,
                    uuid,
                    crate::plugin::api::events::server::whitelist_state_update::WhitelistStateUpdateStatus::Added,
                );
            server.plugin_manager.fire_blocking(server, &mut event);
        }

        if successes == 0 {
            Err(ERROR_ADD_FAILED.create_without_context())
        } else {
            Ok(successes)
        }
    }
}

struct RemoveSuggestionProvider;

impl SuggestionProvider for RemoveSuggestionProvider {
    fn suggest(
        &self,
        context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        let whitelist = context.server().data.whitelist_config.read().unwrap();
        for entry in &whitelist.whitelist {
            builder = builder.suggest(entry.name.clone());
        }
        builder.build()
    }
}

struct RemoveExecutor;

impl CommandExecutor for RemoveExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = GameProfileArgumentType::get(context, "targets")?;
        let server = context.source.server();
        let mut whitelist = server.data.whitelist_config.write().unwrap();
        let mut successes: i32 = 0;
        let mut removed: Vec<(String, uuid::Uuid)> = Vec::new();
        for player in &targets {
            let i = whitelist
                .whitelist
                .iter()
                .position(|entry| entry.uuid == player.id);

            if let Some(i) = i {
                whitelist.whitelist.remove(i);
                context.source.send_feedback(
                    papokin_macros::translate!(
                        translation::java::COMMANDS_WHITELIST_REMOVE_SUCCESS,
                        TextComponent::text(player.name.clone())
                    ),
                    true,
                );
                removed.push((player.name.clone(), player.id));
                successes += 1;
            }
        }

        if successes == 0 {
            Err(ERROR_REMOVE_FAILED.create_without_context())
        } else {
            whitelist.save();
            drop(whitelist);

            for (name, uuid) in removed {
                let mut event =
                    crate::plugin::api::events::server::whitelist_state_update::WhitelistStateUpdateEvent::new(
                        name,
                        uuid,
                        crate::plugin::api::events::server::whitelist_state_update::WhitelistStateUpdateStatus::Removed,
                    );
                server.plugin_manager.fire_blocking(server, &mut event);
            }

            kick_non_whitelisted_players(server);

            Ok(successes)
        }
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Three),
    ));

    dispatcher.register(
        command("whitelist", DESCRIPTION)
            .requires(PERMISSION)
            .then(literal("on").executes(OnExecutor))
            .then(literal("off").executes(OffExecutor))
            .then(literal("list").executes(ListExecutor))
            .then(
                literal("add").then(
                    argument("targets", GameProfileArgumentType)
                        .suggests(AddSuggestionProvider)
                        .executes(AddExecutor),
                ),
            )
            .then(
                literal("remove").then(
                    argument("targets", GameProfileArgumentType)
                        .suggests(RemoveSuggestionProvider)
                        .executes(RemoveExecutor),
                ),
            )
            .then(literal("reload").executes(ReloadExecutor)),
    );
}
