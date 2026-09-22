use crate::wit::papokin::plugin::game_rules::{GameRule, GameRuleValue};
use crate::wit::papokin::plugin::world::World;

impl World {
    /// 获取此世界中的布尔型游戏规则值。若该规则为整型规则则返回 `None`。
    pub fn get_game_rule_bool(&self, rule: GameRule) -> Option<bool> {
        match self.get_game_rule(rule) {
            GameRuleValue::Bool(v) => Some(v),
            GameRuleValue::Int(_) => None,
        }
    }

    /// 获取此世界中的整型游戏规则值。若该规则为布尔规则则返回 `None`。
    pub fn get_game_rule_int(&self, rule: GameRule) -> Option<i32> {
        match self.get_game_rule(rule) {
            GameRuleValue::Int(v) => Some(v),
            GameRuleValue::Bool(_) => None,
        }
    }

    /// 为此世界设置布尔型游戏规则值。
    pub fn set_game_rule_bool(&self, rule: GameRule, value: bool) {
        self.set_game_rule(rule, GameRuleValue::Bool(value));
    }

    /// 为此世界设置整型游戏规则值。
    pub fn set_game_rule_int(&self, rule: GameRule, value: i32) {
        self.set_game_rule(rule, GameRuleValue::Int(value));
    }
}
