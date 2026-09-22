use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::dialog::{DialogArg, DialogArgumentType};
use crate::command::argument_types::entity::EntityArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::LiteralCommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use papokin_protocol::IdOr;
use papokin_protocol::java::client::dialog::DialogNBT;
use papokin_protocol::java::client::play::{CPlayClearDialog, CPlayShowDialog};
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

const DESCRIPTION: &str = "管理玩家的对话框界面。";
const PERMISSION: &str = "minecraft:command.dialog";

const ARG_TARGETS: &str = "targets";
const ARG_DIALOG: &str = "dialog";

struct DialogClearExecutor;

impl CommandExecutor for DialogClearExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = EntityArgumentType::get_players(context, ARG_TARGETS)?;

        let count = targets.len();
        let packet = CPlayClearDialog::new();
        for player in &targets {
            player.try_send_client_packet(&packet);
        }

        let msg = if count == 1 {
            TextComponent::text(format!("已清除 {} 的对话框", targets[0].gameprofile.name))
        } else {
            TextComponent::text(format!("已为 {count} 名玩家清除对话框"))
        };
        context.source.send_feedback(msg, true);

        Ok(count as i32)
    }
}

static UNKNOWN_DIALOG_ERROR: LiteralCommandErrorType = LiteralCommandErrorType::new(
    "未知的对话框。请使用已注册的对话框 id，或使用 SNBT 内联指定对话框。",
);

/// 解析对话框 id（例如 `"minecraft:server_links"` 或插件注册的
/// 命名空间 id）映射到其网络 id：原版条目来自静态
/// `dialog` 注册表，来自服务器注册表管理器的自定义条目
/// (id = 原版条目数 + 注册索引，与注册表
/// 同步）。
fn dialog_network_id(name: &str, server: &crate::server::Server) -> Option<u16> {
    let vanilla = papokin_data::registry::REGISTRY_V_26_3
        .iter()
        .find(|registry| registry.registry_id == "dialog")?;
    let path = name.strip_prefix("minecraft:").unwrap_or(name);
    if let Some(index) = vanilla.entries.iter().position(|entry| entry.name == path) {
        return u16::try_from(index).ok();
    }
    let custom_index = server.registry_manager.index_of("dialog", name)?;
    u16::try_from(vanilla.entries.len() + usize::from(custom_index)).ok()
}

struct DialogShowExecutor;

impl CommandExecutor for DialogShowExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let targets = EntityArgumentType::get_players(context, ARG_TARGETS)?;
        let dialog_arg = DialogArgumentType::get(context, ARG_DIALOG)?;

        let dialog = match dialog_arg {
            DialogArg::Nbt(compound) => IdOr::Value(DialogNBT::from_nbt(compound)),
            DialogArg::Id(id) => {
                let network_id = dialog_network_id(&id.to_string(), context.source.server())
                    .ok_or_else(|| UNKNOWN_DIALOG_ERROR.create_without_context())?;
                IdOr::Id(network_id)
            }
        };

        let count = targets.len();
        let packet = CPlayShowDialog::new(dialog);
        for player in &targets {
            player.try_send_client_packet(&packet);
        }

        let msg = if count == 1 {
            TextComponent::text(format!("已向 {} 显示对话框", targets[0].gameprofile.name))
        } else {
            TextComponent::text(format!("已向 {count} 名玩家显示对话框"))
        };
        context.source.send_feedback(msg, true);

        Ok(count as i32)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("dialog", DESCRIPTION)
            .requires(PERMISSION)
            .then(literal("clear").then(
                argument(ARG_TARGETS, EntityArgumentType::Players).executes(DialogClearExecutor),
            ))
            .then(
                literal("show").then(
                    argument(ARG_TARGETS, EntityArgumentType::Players).then(
                        argument(ARG_DIALOG, DialogArgumentType).executes(DialogShowExecutor),
                    ),
                ),
            ),
    );
}
