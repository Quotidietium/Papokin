use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use tracing::error;

use crate::command::argument_builder::{ArgumentBuilder, command, literal};
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "将服务器保存到磁盘。";

const PERMISSION: &str = "minecraft:command.save-all";

struct SaveAllExecutor;

impl CommandExecutor for SaveAllExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        context.source.send_feedback(
            TextComponent::translate(translation::java::COMMANDS_SAVE_SAVING, []),
            false,
        );

        let server_arc = context.server().clone();
        let server_clone = server_arc.clone();
        let source = context.source.clone();
        server_arc.spawn_task(async move {
            if let Err(err) = server_clone.save_all().await {
                error!("保存服务器数据失败：{err}");
                source.send_error(TextComponent::translate(
                    translation::java::COMMANDS_SAVE_FAILED,
                    [],
                ));
            } else {
                source.send_feedback(
                    TextComponent::translate(translation::java::COMMANDS_SAVE_SUCCESS, []),
                    true,
                );
            }
        });

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
        command("save-all", DESCRIPTION)
            .requires(PERMISSION)
            .executes(SaveAllExecutor)
            .then(literal("flush").executes(SaveAllExecutor)),
    );
}
