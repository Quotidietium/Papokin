use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

use crate::command::argument_builder::{ArgumentBuilder, command};
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "重载服务器的数据包。";
const PERMISSION: &str = "minecraft:command.reload";

struct ReloadExecutor;

impl CommandExecutor for ReloadExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        // 原版在执行重载前先发布公告，因此反馈
        // 即便重载需要片刻也会到达。
        context.source.send_feedback(
            TextComponent::translate(translation::java::COMMANDS_RELOAD_SUCCESS, []),
            true,
        );

        let server = context.server().clone();
        server.reload_datapacks(&server);

        Ok(0)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("reload", DESCRIPTION)
            .requires(PERMISSION)
            .executes(ReloadExecutor),
    );
}
