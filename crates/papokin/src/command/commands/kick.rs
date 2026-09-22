use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use papokin_util::text::color::NamedColor;

use crate::command::argument_builder::{ArgumentBuilder, argument, command};
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::argument_types::entity::EntityArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::entity::EntityBase;

const DESCRIPTION: &str = "将目标玩家踢出服务器。";
const PERMISSION: &str = "minecraft:command.kick";

struct KickExecutor {
    has_reason: bool,
}

impl CommandExecutor for KickExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = EntityArgumentType::get_players(context, "targets")?;

        let reason = if self.has_reason {
            let custom_reason = StringArgumentType::get(context, "reason")?;
            TextComponent::text(custom_reason.to_string())
        } else {
            TextComponent::translate(translation::java::MULTIPLAYER_DISCONNECT_KICKED, [])
        };

        for target in &targets {
            target.kick(&reason);

            let feedback = TextComponent::translate(
                translation::java::COMMANDS_KICK_SUCCESS,
                [target.as_ref().get_display_name(), reason.clone()],
            );

            context
                .source
                .send_feedback(feedback.color_named(NamedColor::Blue), true);
        }

        Ok(targets.len() as i32)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Three),
    ));

    dispatcher.register(
        command("kick", DESCRIPTION).requires(PERMISSION).then(
            argument("targets", EntityArgumentType::Players)
                .executes(KickExecutor { has_reason: false })
                .then(
                    argument("reason", StringArgumentType::GreedyPhrase)
                        .executes(KickExecutor { has_reason: true }),
                ),
        ),
    );
}
