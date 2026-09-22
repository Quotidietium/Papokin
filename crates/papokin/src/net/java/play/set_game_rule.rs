#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_data::game_rules::{GameRule, GameRuleValue};
use papokin_protocol::java::server::play::SSetGameRule;

impl JavaClient {
    pub fn handle_set_game_rule(&self, player: &Player, packet: &SSetGameRule<'_>) {
        if player.permission_lvl.load() < PermissionLvl::Two {
            warn!(
                "玩家 {} 试图设置游戏规则，但缺少所需权限",
                player.gameprofile.name
            );
            return;
        }

        let world = player.world();
        let minecart_improvements_enabled = world.server.upgrade().map_or_else(
            || {
                world
                    .level_info
                    .load()
                    .data_packs
                    .enabled
                    .iter()
                    .any(|p| p == "minecart_improvements" || p == "file/minecart_improvements")
            },
            |s| s.is_feature_enabled("minecraft:minecart_improvements"),
        );

        for entry in &packet.entries {
            let key = entry
                .game_rule_key
                .strip_prefix("minecraft:")
                .unwrap_or(entry.game_rule_key);
            let Some(rule) = GameRule::all().iter().find(|r| r.to_string() == key) else {
                warn!("未知的游戏规则：{}", entry.game_rule_key);
                continue;
            };

            if *rule == GameRule::MaxMinecartSpeed && !minecart_improvements_enabled {
                warn!(
                    "玩家 {} 试图设置未被特性标志启用的游戏规则 {}",
                    player.gameprofile.name, entry.game_rule_key
                );
                continue;
            }

            let level_info = player.world().level_info.load();
            let current_val = level_info.game_rules.get(rule);
            match current_val {
                GameRuleValue::Int(_) => {
                    if let Ok(val) = entry.value.parse::<i64>() {
                        player.world().set_game_rule(rule, GameRuleValue::Int(val));
                        info!(
                            "玩家 {} 将游戏规则 {} 设置为 {}",
                            player.gameprofile.name, key, val
                        );
                    } else {
                        warn!(
                            "玩家 {} 试图为游戏规则 {} 设置无效的整数值 '{}'",
                            player.gameprofile.name, entry.value, key
                        );
                    }
                }
                GameRuleValue::Bool(_) => {
                    if let Ok(val) = entry.value.parse::<bool>() {
                        player.world().set_game_rule(rule, GameRuleValue::Bool(val));
                        info!(
                            "玩家 {} 将游戏规则 {} 设置为 {}",
                            player.gameprofile.name, key, val
                        );
                    } else {
                        warn!(
                            "玩家 {} 试图为游戏规则 {} 设置无效的布尔值 '{}'",
                            player.gameprofile.name, entry.value, key
                        );
                    }
                }
            }
        }
    }
}
