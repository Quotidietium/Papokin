use std::sync::atomic::Ordering;

use papokin_util::{
    PermissionLvl,
    permission::{Permission, PermissionDefault, PermissionRegistry},
    text::TextComponent,
};

use crate::command::{
    argument_builder::{ArgumentBuilder, argument, command},
    argument_types::core::integer::IntegerArgumentType,
    context::command_context::CommandContext,
    node::{CommandExecutor, CommandExecutorResult, dispatcher::CommandDispatcher},
};

const DESCRIPTION: &str = "设置空闲玩家被踢出服务器前的等待时长。";
const PERMISSION: &str = "minecraft:command.setidletimeout";

const ARG_MINUTES: &str = "minutes";

struct SetIdleTimeoutExecutor;

impl CommandExecutor for SetIdleTimeoutExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let minutes: i32 = IntegerArgumentType::get(context, ARG_MINUTES)?;

        context
            .server()
            .player_idle_timeout
            .store(minutes, Ordering::Relaxed);

        if minutes == 0 {
            context.source.send_feedback(
                TextComponent::translate("commands.setidletimeout.success.disabled", []),
                true,
            );
        } else {
            context.source.send_feedback(
                TextComponent::translate(
                    "commands.setidletimeout.success",
                    [TextComponent::text(minutes.to_string())],
                ),
                true,
            );
        }

        Ok(minutes)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Three),
    ));

    dispatcher.register(
        command("setidletimeout", DESCRIPTION)
            .requires(PERMISSION)
            .then(
                argument(ARG_MINUTES, IntegerArgumentType::with_min(0))
                    .executes(SetIdleTimeoutExecutor),
            ),
    );
}
