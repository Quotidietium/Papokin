use std::sync::Arc;

use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::CTransfer as JavaCTransfer;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use tracing::info;

use crate::command::argument_builder::{ArgumentBuilder, argument, command};
use crate::command::argument_types::core::integer::IntegerArgumentType;
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::argument_types::entity::EntityArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::context::command_source::CommandSource;
use crate::command::errors::command_syntax_error::CommandSyntaxError;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::entity::EntityBase;
use crate::entity::player::Player;

const DESCRIPTION: &str = "触发将一名玩家转移到另一台服务器。";
const PERMISSION: &str = "minecraft:command.transfer";

const ERROR_NOT_PLAYER: CommandErrorType<0> =
    CommandErrorType::new(papokin_data::translation::java::PERMISSIONS_REQUIRES_PLAYER);

fn transfer_players(
    source: &CommandSource,
    hostname: &str,
    port: i32,
    players: &[Arc<Player>],
) -> Result<i32, CommandSyntaxError> {
    if players.is_empty() {
        return Err(ERROR_NOT_PLAYER.create_without_context());
    }

    for p in players {
        let java_packet = JavaCTransfer::new(hostname, VarInt(port));
        if let Ok(data) = p.client.serialize_packet(&java_packet) {
            p.client.try_enqueue_packet(data);
        }

        info!(
            "[{}：正在将 {} 转移到 {hostname}:{port}]",
            source.name, p.gameprofile.name
        );
    }

    if players.len() == 1 {
        source.send_feedback(
            TextComponent::translate(
                "commands.transfer.success.single",
                [
                    players[0].as_ref().get_display_name(),
                    TextComponent::text(hostname.to_string()),
                    TextComponent::text(port.to_string()),
                ],
            ),
            true,
        );
    } else {
        source.send_feedback(
            TextComponent::translate(
                "commands.transfer.success.multiple",
                [
                    TextComponent::text(players.len().to_string()),
                    TextComponent::text(hostname.to_string()),
                    TextComponent::text(port.to_string()),
                ],
            ),
            true,
        );
    }

    Ok(players.len() as i32)
}

#[derive(Clone, Copy)]
enum TransferStep {
    HostOnly,
    HostAndPort,
    Full,
}

struct TransferExecutor {
    step: TransferStep,
}

impl CommandExecutor for TransferExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let hostname = StringArgumentType::get(context, "hostname")?;

        let port = match self.step {
            TransferStep::HostOnly => 25565,
            TransferStep::HostAndPort | TransferStep::Full => {
                IntegerArgumentType::get(context, "port")?
            }
        };

        let players = match self.step {
            TransferStep::HostOnly | TransferStep::HostAndPort => {
                let player = context
                    .source
                    .output
                    .as_player()
                    .ok_or_else(|| ERROR_NOT_PLAYER.create_without_context())?;
                vec![player]
            }
            TransferStep::Full => EntityArgumentType::get_players(context, "players")?,
        };

        transfer_players(&context.source, hostname, port, &players)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Three),
    ));

    dispatcher.register(
        command("transfer", DESCRIPTION).requires(PERMISSION).then(
            argument("hostname", StringArgumentType::SingleWord)
                .executes(TransferExecutor {
                    step: TransferStep::HostOnly,
                })
                .then(
                    argument("port", IntegerArgumentType::new(1, 65535))
                        .executes(TransferExecutor {
                            step: TransferStep::HostAndPort,
                        })
                        .then(argument("players", EntityArgumentType::Players).executes(
                            TransferExecutor {
                                step: TransferStep::Full,
                            },
                        )),
                ),
        ),
    );
}
