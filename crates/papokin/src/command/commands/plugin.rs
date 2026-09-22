use std::path::Path;

use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use papokin_util::text::color::NamedColor;
use papokin_util::text::hover::HoverEvent;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "管理服务器插件。";
const PERMISSION: &str = "papokin:command.plugin";

struct ListExecutor;

impl CommandExecutor for ListExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server_arc = context.server().clone();
        let plugins = server_arc.plugin_manager.active_plugins();
        let loaded_plugins = server_arc.plugin_manager.loaded_plugins();

        let mut message = TextComponent::text(format!("插件（{}）：", loaded_plugins.len()))
            .color_named(NamedColor::Gold)
            .add_child(TextComponent::text("\n"));

        for (i, plugin) in plugins.iter().enumerate() {
            let metadata = plugin;
            let version = metadata
                .version
                .strip_prefix('v')
                .unwrap_or(&metadata.version);
            let line = if i == plugins.len() - 1 {
                format!("- {} (v{version})", metadata.name)
            } else {
                format!("- {} (v{version})\n", metadata.name)
            };
            let hover_text = format!(
                "版本：{}\n作者：{}\n描述：{}",
                metadata.version,
                metadata.authors.join(", "),
                metadata.description
            );
            let mut plugin_component = TextComponent::text(line)
                .color_named(NamedColor::Green)
                .hover_event(HoverEvent::show_text(TextComponent::text(hover_text)));

            if !metadata.permissions.is_empty() {
                plugin_component = plugin_component.add_child(
                    TextComponent::text(format!("（权限：{:?}）", metadata.permissions))
                        .color_named(NamedColor::Gray),
                );
            }

            message = message.add_child(plugin_component);
        }

        context.source.send_feedback(message, false);

        Ok(1)
    }
}

struct LoadExecutor;

impl CommandExecutor for LoadExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let plugin_name = StringArgumentType::get(context, "plugin")?.to_string();
        let server_arc = context.server().clone();

        if server_arc.plugin_manager.is_plugin_active(&plugin_name) {
            context.source.send_feedback(
                TextComponent::text(format!("插件 {plugin_name} 已加载")),
                false,
            );
            return Ok(1);
        }

        let source_clone = context.source.clone();
        let plugin_name_clone = plugin_name;
        let server_clone = server_arc.clone();
        server_arc.spawn_task(async move {
            let result = server_clone
                .plugin_manager
                .try_load_plugin(&server_clone, Path::new(&plugin_name_clone))
                .await;

            match result {
                Ok(()) => {
                    source_clone.send_feedback(
                        TextComponent::text(format!("插件 {plugin_name_clone} 加载成功"))
                            .color_named(NamedColor::Green),
                        true,
                    );
                }
                Err(e) => {
                    source_clone.send_feedback(
                        TextComponent::text(format!("加载插件 {plugin_name_clone} 失败：{e}")),
                        false,
                    );
                }
            }
        });

        Ok(1)
    }
}

struct UnloadExecutor;

impl CommandExecutor for UnloadExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let plugin_name = StringArgumentType::get(context, "plugin")?.to_string();
        let server_arc = context.server().clone();

        if !server_arc.plugin_manager.is_plugin_active(&plugin_name) {
            context.source.send_feedback(
                TextComponent::text(format!("插件 {plugin_name} 未加载")),
                false,
            );
            return Ok(1);
        }

        let source_clone = context.source.clone();
        let plugin_name_clone = plugin_name;
        let server_clone = server_arc.clone();
        server_arc.spawn_task(async move {
            let result = server_clone
                .plugin_manager
                .unload_plugin(&plugin_name_clone)
                .await;

            match result {
                Ok(()) => {
                    source_clone.send_feedback(
                        TextComponent::text(format!("插件 {plugin_name_clone} 卸载成功"))
                            .color_named(NamedColor::Green),
                        true,
                    );
                }
                Err(e) => {
                    source_clone.send_feedback(
                        TextComponent::text(format!("卸载插件 {plugin_name_clone} 失败：{e}")),
                        false,
                    );
                }
            }
        });

        Ok(1)
    }
}

struct HotReloadExecutor(bool);

impl CommandExecutor for HotReloadExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let enabled = self.0;
        let server_arc = context.server().clone();
        let source_clone = context.source.clone();
        let server_clone = server_arc.clone();

        if enabled {
            server_arc.spawn_task(async move {
                if let Err(e) = server_clone
                    .plugin_manager
                    .start_watcher(&server_clone)
                    .await
                {
                    source_clone.send_feedback(
                        TextComponent::text(format!("启动插件监视器失败：{e}")),
                        false,
                    );
                    return;
                }

                source_clone.send_feedback(
                    TextComponent::text("热重载已启用。").color_named(NamedColor::Green),
                    true,
                );
                source_clone.send_feedback(
                    TextComponent::text("警告：热重载会影响性能，只应在插件开发期间启用。")
                        .color_named(NamedColor::Red),
                    false,
                );
            });
        } else {
            server_arc.spawn_task(async move {
                server_clone.plugin_manager.stop_watcher().await;
                source_clone.send_feedback(
                    TextComponent::text("热重载已禁用。").color_named(NamedColor::Yellow),
                    true,
                );
            });
        }

        Ok(1)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Three),
    ));

    dispatcher.register(
        command("plugin", DESCRIPTION)
            .requires(PERMISSION)
            .then(literal("list").executes(ListExecutor))
            .then(
                literal("load").then(
                    argument("plugin", StringArgumentType::SingleWord).executes(LoadExecutor),
                ),
            )
            .then(
                literal("unload").then(
                    argument("plugin", StringArgumentType::SingleWord).executes(UnloadExecutor),
                ),
            )
            .then(
                literal("hotreload")
                    .then(literal("enable").executes(HotReloadExecutor(true)))
                    .then(literal("disable").executes(HotReloadExecutor(false))),
            ),
    );
}
