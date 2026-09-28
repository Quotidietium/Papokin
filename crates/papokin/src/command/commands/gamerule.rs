use std::sync::Arc;

use papokin_data::game_rules::{GameRule, GameRuleRegistry, GameRuleValue};
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::core::bool::BoolArgumentType;
use crate::command::argument_types::core::integer::IntegerArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::plugin::api::events::world::world_game_rule_change::WorldGameRuleChangeEvent;

const DESCRIPTION: &str = "设置或查询一条游戏规则的值。";
const PERMISSION: &str = "minecraft:command.gamerule";

/// 通知插件游戏规则即将更改；返回 `true` 表示被插件取消，
/// 调用方不得写入。
fn fire_game_rule_change(context: &CommandContext, rule: &GameRule, value: String) -> bool {
    let server = context.source.server().clone();
    let world = context.source.world().clone();
    let mut event = WorldGameRuleChangeEvent::new(world, rule.to_string(), value);
    server.plugin_manager.fire_blocking(&server, &mut event);
    event.cancelled
}

struct QueryExecutor(GameRule);

impl CommandExecutor for QueryExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let key = TextComponent::text(self.0.to_string());
        let level_info = context.server().level_info.load();
        let game_rule = level_info.game_rules.get(&self.0);
        let game_rule_i32_value = match game_rule {
            GameRuleValue::Int(value) => (*value).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            GameRuleValue::Bool(value) => *value as i32,
        };
        let value = TextComponent::text(game_rule.to_string());
        drop(level_info);

        context.source.send_feedback(
            TextComponent::translate("commands.gamerule.query", [key, value]),
            false,
        );

        Ok(game_rule_i32_value)
    }
}

struct SetIntExecutor(GameRule);

impl CommandExecutor for SetIntExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let key = TextComponent::text(self.0.to_string());
        let arg_value = IntegerArgumentType::get(context, "value")? as i64;

        let current_info = context.server().level_info.load();
        let mut new_info = (**current_info).clone();

        // 先发事件并尊重取消：此前写入已生效后才通知，取消被无视
        if fire_game_rule_change(context, &self.0, arg_value.to_string()) {
            return Ok(arg_value.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }

        if let GameRuleValue::Int(raw_value) = new_info.game_rules.get_mut(&self.0) {
            *raw_value = arg_value;
        }

        context.server().level_info.store(Arc::new(new_info));

        let value_component = TextComponent::text(arg_value.to_string());
        context.source.send_feedback(
            TextComponent::translate("commands.gamerule.set", [key, value_component]),
            true,
        );

        Ok(arg_value.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
    }
}

struct SetBoolExecutor(GameRule);

impl CommandExecutor for SetBoolExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let key = TextComponent::text(self.0.to_string());
        let arg_value = BoolArgumentType::get(context, "value")?;

        let current_info = context.server().level_info.load();
        let mut new_info = (**current_info).clone();

        // 先发事件并尊重取消：此前写入已生效后才通知，取消被无视
        if fire_game_rule_change(context, &self.0, arg_value.to_string()) {
            return Ok(arg_value as i32);
        }

        if let GameRuleValue::Bool(raw_value) = new_info.game_rules.get_mut(&self.0) {
            *raw_value = arg_value;
        }

        context.server().level_info.store(Arc::new(new_info));

        if self.0 == GameRule::SpectatorsGenerateChunks {
            let server = context.server();
            for world in server.worlds.load().iter() {
                for player in world.players.load().iter() {
                    if player.is_spectator() {
                        player.update_chunk_tickets_for_gamemode();
                    }
                }
            }
        }

        let value_component = TextComponent::text(arg_value.to_string());
        context.source.send_feedback(
            TextComponent::translate("commands.gamerule.set", [key, value_component]),
            true,
        );

        Ok(arg_value as i32)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    let mut cmd = command("gamerule", DESCRIPTION).requires(PERMISSION);
    let rule_registry = GameRuleRegistry::default();

    for rule in GameRule::all() {
        let rule_literal = literal(rule.to_string()).executes(QueryExecutor(rule.clone()));
        let branch = match rule_registry.get(rule) {
            GameRuleValue::Int(_) => rule_literal.then(
                argument("value", IntegerArgumentType::any())
                    .executes(SetIntExecutor(rule.clone())),
            ),
            GameRuleValue::Bool(_) => rule_literal
                .then(argument("value", BoolArgumentType).executes(SetBoolExecutor(rule.clone()))),
        };
        cmd = cmd.then(branch);
    }

    dispatcher.register(cmd);
}
