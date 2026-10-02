use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use papokin_util::text::color::NamedColor;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::command::suggestion::provider::{SuggestionProvider, SuggestionProviderResult};
use crate::command::suggestion::suggestions::SuggestionsBuilder;
use crate::plugin::PLUGIN_DIR;

const DESCRIPTION: &str = "动态管理插件：启用、禁用、加载与卸载。";
const PERMISSION: &str = "papokin:command.plugman";

/// 将用户输入的插件文件名解析为插件目录内的路径。
/// 只接受裸文件名：拒绝空串、绝对路径以及任何含分隔符或
/// `..` 的输入，防止目录穿越到插件目录之外。
fn resolve_plugin_file(file_name: &str) -> Result<PathBuf, String> {
    let mut components = Path::new(file_name).components();
    let is_bare_name = !file_name.is_empty()
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none();
    if is_bare_name {
        Ok(Path::new(PLUGIN_DIR).join(file_name))
    } else {
        Err(format!(
            "无效的插件文件名“{file_name}”：只允许插件目录内的裸文件名"
        ))
    }
}

/// 建议当前处于启用状态的插件名（`disable` 用）。
struct EnabledPluginSuggestions;

impl SuggestionProvider for EnabledPluginSuggestions {
    fn suggest(
        &self,
        context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        for metadata in context.server().plugin_manager.active_plugins() {
            builder = builder.suggest(metadata.name);
        }
        builder.build()
    }
}

/// 建议已加载但未启用的插件名（`enable` 用）。
struct DisabledPluginSuggestions;

impl SuggestionProvider for DisabledPluginSuggestions {
    fn suggest(
        &self,
        context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        let manager = &context.server().plugin_manager;
        let active: HashSet<String> = manager
            .active_plugins()
            .iter()
            .map(|metadata| metadata.name.clone())
            .collect();
        for metadata in manager.loaded_plugins() {
            if !active.contains(&metadata.name) {
                builder = builder.suggest(metadata.name);
            }
        }
        builder.build()
    }
}

/// 建议全部已加载（含已禁用）的插件名（`unload` 用）。
struct LoadedPluginSuggestions;

impl SuggestionProvider for LoadedPluginSuggestions {
    fn suggest(
        &self,
        context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        for metadata in context.server().plugin_manager.loaded_plugins() {
            builder = builder.suggest(metadata.name);
        }
        builder.build()
    }
}

/// 建议插件目录内的文件名（`load` 用）。
struct PluginFileSuggestions;

impl SuggestionProvider for PluginFileSuggestions {
    fn suggest(
        &self,
        _context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        if let Ok(entries) = std::fs::read_dir(PLUGIN_DIR) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(name) = entry.file_name().to_str()
                {
                    builder = builder.suggest(name.to_string());
                }
            }
        }
        builder.build()
    }
}

struct EnableExecutor;

impl CommandExecutor for EnableExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let plugin_name = StringArgumentType::get(context, "plugin")?.to_string();
        let server = context.server().clone();

        if !server.plugin_manager.is_plugin_loaded(&plugin_name) {
            context.source.send_feedback(
                TextComponent::text(format!("插件 {plugin_name} 未加载"))
                    .color_named(NamedColor::Red),
                false,
            );
            return Ok(1);
        }
        if server.plugin_manager.is_plugin_active(&plugin_name) {
            context.source.send_feedback(
                TextComponent::text(format!("插件 {plugin_name} 已处于启用状态")),
                false,
            );
            return Ok(1);
        }

        let source = context.source.clone();
        let server_clone = server.clone();
        server.spawn_task(async move {
            match server_clone
                .plugin_manager
                .enable_plugin(&plugin_name)
                .await
            {
                Ok(()) => {
                    source.send_feedback(
                        TextComponent::text(format!("插件 {plugin_name} 已启用"))
                            .color_named(NamedColor::Green),
                        true,
                    );
                }
                Err(e) => {
                    source.send_feedback(
                        TextComponent::text(format!("启用插件 {plugin_name} 失败：{e}"))
                            .color_named(NamedColor::Red),
                        false,
                    );
                }
            }
        });

        Ok(1)
    }
}

struct DisableExecutor;

impl CommandExecutor for DisableExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let plugin_name = StringArgumentType::get(context, "plugin")?.to_string();
        let server = context.server().clone();

        if !server.plugin_manager.is_plugin_active(&plugin_name) {
            context.source.send_feedback(
                TextComponent::text(format!("插件 {plugin_name} 未处于启用状态"))
                    .color_named(NamedColor::Red),
                false,
            );
            return Ok(1);
        }

        let source = context.source.clone();
        let server_clone = server.clone();
        server.spawn_task(async move {
            match server_clone
                .plugin_manager
                .disable_plugin(&plugin_name)
                .await
            {
                Ok(()) => {
                    source.send_feedback(
                        TextComponent::text(format!("插件 {plugin_name} 已禁用"))
                            .color_named(NamedColor::Yellow),
                        true,
                    );
                }
                Err(e) => {
                    source.send_feedback(
                        TextComponent::text(format!("禁用插件 {plugin_name} 失败：{e}"))
                            .color_named(NamedColor::Red),
                        false,
                    );
                }
            }
        });

        Ok(1)
    }
}

struct LoadExecutor;

impl CommandExecutor for LoadExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let file_name = StringArgumentType::get(context, "file")?.to_string();
        let server = context.server().clone();

        let path = match resolve_plugin_file(&file_name) {
            Ok(path) => path,
            Err(message) => {
                context.source.send_feedback(
                    TextComponent::text(message).color_named(NamedColor::Red),
                    false,
                );
                return Ok(1);
            }
        };
        if !path.is_file() {
            context.source.send_feedback(
                TextComponent::text(format!("插件目录中不存在文件 {file_name}"))
                    .color_named(NamedColor::Red),
                false,
            );
            return Ok(1);
        }

        let source = context.source.clone();
        let server_clone = server.clone();
        server.spawn_task(async move {
            match server_clone
                .plugin_manager
                .try_load_plugin(&server_clone, &path)
                .await
            {
                Ok(()) => {
                    source.send_feedback(
                        TextComponent::text(format!("插件文件 {file_name} 加载成功"))
                            .color_named(NamedColor::Green),
                        true,
                    );
                }
                Err(e) => {
                    source.send_feedback(
                        TextComponent::text(format!("加载插件文件 {file_name} 失败：{e}"))
                            .color_named(NamedColor::Red),
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
        let server = context.server().clone();

        if !server.plugin_manager.is_plugin_loaded(&plugin_name) {
            context.source.send_feedback(
                TextComponent::text(format!("插件 {plugin_name} 未加载"))
                    .color_named(NamedColor::Red),
                false,
            );
            return Ok(1);
        }

        let source = context.source.clone();
        let server_clone = server.clone();
        server.spawn_task(async move {
            match server_clone
                .plugin_manager
                .unload_plugin(&plugin_name)
                .await
            {
                Ok(()) => {
                    source.send_feedback(
                        TextComponent::text(format!("插件 {plugin_name} 已卸载"))
                            .color_named(NamedColor::Green),
                        true,
                    );
                }
                Err(e) => {
                    source.send_feedback(
                        TextComponent::text(format!("卸载插件 {plugin_name} 失败：{e}"))
                            .color_named(NamedColor::Red),
                        false,
                    );
                }
            }
        });

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
        command("plugman", DESCRIPTION)
            .requires(PERMISSION)
            .then(
                literal("enable").then(
                    argument("plugin", StringArgumentType::SingleWord)
                        .suggests(DisabledPluginSuggestions)
                        .executes(EnableExecutor),
                ),
            )
            .then(
                literal("disable").then(
                    argument("plugin", StringArgumentType::SingleWord)
                        .suggests(EnabledPluginSuggestions)
                        .executes(DisableExecutor),
                ),
            )
            .then(
                literal("load").then(
                    argument("file", StringArgumentType::SingleWord)
                        .suggests(PluginFileSuggestions)
                        .executes(LoadExecutor),
                ),
            )
            .then(
                literal("unload").then(
                    argument("plugin", StringArgumentType::SingleWord)
                        .suggests(LoadedPluginSuggestions)
                        .executes(UnloadExecutor),
                ),
            ),
    );
}

#[cfg(test)]
mod tests {
    use super::resolve_plugin_file;
    use crate::plugin::PLUGIN_DIR;
    use std::path::Path;

    #[test]
    fn resolve_plugin_file_accepts_bare_names() {
        let path = resolve_plugin_file("myplugin.wasm").unwrap();
        assert_eq!(path, Path::new(PLUGIN_DIR).join("myplugin.wasm"));
    }

    #[test]
    fn resolve_plugin_file_rejects_traversal_and_non_bare_names() {
        for bad in [
            "",
            ".",
            "..",
            "../evil.wasm",
            "dir/plugin.wasm",
            r"dir\plugin.wasm",
            "/abs.wasm",
            r"C:\abs.wasm",
        ] {
            assert!(resolve_plugin_file(bad).is_err(), "应当拒绝: {bad}");
        }
    }
}
