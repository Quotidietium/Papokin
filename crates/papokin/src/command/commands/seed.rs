use crate::command::argument_builder::{ArgumentBuilder, command};
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::click::ClickEvent;
use papokin_util::text::hover::HoverEvent;
use papokin_util::text::{TextComponent, color::NamedColor};
use std::borrow::Cow;

const DESCRIPTION: &str = "显示世界种子。";
const PERMISSION: &str = "minecraft:command.seed";

struct SeedCommandExecutor;

fn create_copy_on_click_text(content: String) -> TextComponent {
    TextComponent::translate(
        translation::java::COMMANDS_SEED_SUCCESS,
        [TextComponent::wrap_in_square_brackets(
            TextComponent::text(content.clone())
                .hover_event(HoverEvent::show_text(TextComponent::translate(
                    translation::java::CHAT_COPY_CLICK,
                    [],
                )))
                .click_event(ClickEvent::CopyToClipboard {
                    value: Cow::from(content),
                })
                .color_named(NamedColor::Green),
        )],
    )
}

impl CommandExecutor for SeedCommandExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let seed = context.world().level.seed.0;
        let seed_string = seed.to_string();

        context
            .source
            .send_feedback(create_copy_on_click_text(seed_string), false);

        Ok(seed as i32)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        // 对于内置服务器，权限等级为 0，
        // 但 Pumpkin 始终是专用服务器。对于专用服务器，
        // /seed 仅限权限等级 2。
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("seed", DESCRIPTION)
            .requires(PERMISSION)
            .executes(SeedCommandExecutor),
    );
}
