use crate::command::argument_builder::{ArgumentBuilder, argument, command};
use crate::command::argument_types::entity::EntityArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

const DESCRIPTION: &str = "杀死所有目标实体。";

const PERMISSION: &str = "minecraft:command.kill";

const ARG_TARGETS: &str = "targets";

struct TargetsExecutor;

impl CommandExecutor for TargetsExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = EntityArgumentType::get_entities(context, ARG_TARGETS)?;

        let target_count = targets.len();
        for target in &targets {
            target.kill(target.as_ref());
        }

        let msg = if target_count == 1 {
            TextComponent::translate(
                translation::java::COMMANDS_KILL_SUCCESS_SINGLE,
                [targets[0].get_display_name()],
            )
        } else {
            TextComponent::translate(
                translation::java::COMMANDS_KILL_SUCCESS_MULTIPLE,
                [TextComponent::text(target_count.to_string())],
            )
        };

        context.source.send_feedback(msg, true);

        Ok(target_count as i32)
    }
}

struct SelfExecutor;

impl CommandExecutor for SelfExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let target = context.source.entity_or_err()?;
        target.kill(&*target);

        context.source.send_feedback(
            TextComponent::translate(
                translation::java::COMMANDS_KILL_SUCCESS_SINGLE,
                [target.get_display_name()],
            ),
            true,
        );

        Ok(1)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("kill", DESCRIPTION)
            .requires(PERMISSION)
            .then(argument(ARG_TARGETS, EntityArgumentType::Entities).executes(TargetsExecutor))
            .executes(SelfExecutor),
    );
}
