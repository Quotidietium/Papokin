use std::sync::atomic::Ordering::Relaxed;

use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

use crate::command::argument_builder::{ArgumentBuilder, command};
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "禁用服务器自动保存。";

const PERMISSION: &str = "minecraft:command.save-off";

const ALREADY_OFF_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_SAVE_ALREADYOFF);

struct SaveOffExecutor;

impl CommandExecutor for SaveOffExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let mut any_disabled = false;
        for world in context.server().worlds.load().iter() {
            if world.level.save_enabled.swap(false, Relaxed) {
                any_disabled = true;
            }
        }

        if !any_disabled {
            return Err(ALREADY_OFF_ERROR_TYPE.create_without_context());
        }

        context.source.send_feedback(
            TextComponent::translate(translation::java::COMMANDS_SAVE_DISABLED, []),
            true,
        );

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
        command("save-off", DESCRIPTION)
            .requires(PERMISSION)
            .executes(SaveOffExecutor),
    );
}
