use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

use crate::command::argument_builder::{ArgumentBuilder, argument, command};
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::argument_types::game_profile::GameProfileArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::data::SaveJSONConfiguration;
use crate::data::banlist_serializer::BannedPlayerEntry;
use crate::net::GameProfile;

const DESCRIPTION: &str = "封禁一名玩家。";
const PERMISSION: &str = "minecraft:command.ban";

const ERROR_BAN_FAILED: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_BAN_FAILED);

fn ban_profile(context: &CommandContext, profile: &GameProfile, reason: Option<String>) -> bool {
    let server = context.source.server();
    let mut banned_players = server.data.banned_player_list.write().unwrap();

    let reason = reason.unwrap_or_else(|| "被管理员封禁。".to_string());

    if let Some(entry) = banned_players
        .banned_players
        .iter_mut()
        .find(|entry| entry.uuid == profile.id)
    {
        if entry.name != profile.name {
            entry.name.clone_from(&profile.name);
            banned_players.save();
        }
        return false;
    }

    banned_players.banned_players.push(BannedPlayerEntry::new(
        profile,
        context.source.name.clone(),
        None,
        reason.clone(),
    ));

    banned_players.save();
    drop(banned_players);

    context.source.send_feedback(
        TextComponent::translate(
            translation::java::COMMANDS_BAN_SUCCESS,
            [
                TextComponent::text(profile.name.clone()),
                TextComponent::text(reason),
            ],
        ),
        true,
    );

    if let Some(player) = server.get_player_by_uuid(profile.id) {
        let kick_msg =
            TextComponent::translate(translation::java::MULTIPLAYER_DISCONNECT_BANNED, []);
        player.kick(&kick_msg);
    }

    true
}

struct BanExecutor {
    has_reason: bool,
}

impl CommandExecutor for BanExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = GameProfileArgumentType::get(context, "targets")?;
        let reason = if self.has_reason {
            Some(StringArgumentType::get(context, "reason")?.to_string())
        } else {
            None
        };

        let mut count: usize = 0;
        for target in &targets {
            if ban_profile(context, target, reason.clone()) {
                count += 1;
            }
        }

        if count == 0 {
            Err(ERROR_BAN_FAILED.create_without_context())
        } else {
            Ok(count as i32)
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
        command("ban", DESCRIPTION).requires(PERMISSION).then(
            argument("targets", GameProfileArgumentType)
                .executes(BanExecutor { has_reason: false })
                .then(
                    argument("reason", StringArgumentType::GreedyPhrase)
                        .executes(BanExecutor { has_reason: true }),
                ),
        ),
    );
}
