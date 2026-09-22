use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use papokin_util::text::color::NamedColor;

use crate::command::argument_builder::{ArgumentBuilder, command};
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::stop_server;

const DESCRIPTION: &str = "停止服务器。";

const PERMISSION: &str = "minecraft:command.stop";

struct StopCommandExecutor;

impl CommandExecutor for StopCommandExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        context.source.send_feedback(
            TextComponent::translate(translation::java::COMMANDS_STOP_STOPPING, [])
                .color_named(NamedColor::Red),
            true,
        );
        stop_server();
        Ok(1)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Four),
    ));

    dispatcher.register(
        command("stop", DESCRIPTION)
            .requires(PERMISSION)
            .executes(StopCommandExecutor),
    );
}
