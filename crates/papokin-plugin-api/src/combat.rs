//! 生物实体的只读战斗追踪查询。
//!
//! 每个生物实体都维护一个战斗追踪器，记录
//! 其当前战斗回合的伤害条目、击杀归属与战斗状态。这些查询
//! 是 [`LivingEntity`] 句柄上的 WIT 方法
//! （`living.get_combat_entries()`、`living.get_killer()` 等）；本模块
//! 再导出 [`CombatEntry`] 记录并提供 [`PlayerCombatExt`]，使
//! 持有 [`Player`] 的插件无需手动转换
//! 句柄，即可执行同样的查询。
//!
//! 所有时间戳与时长均以毫秒计，源自世界
//! 刻时钟（20 刻/秒，1 刻 = 50 毫秒）。

pub use crate::wit::papokin::plugin::combat::CombatEntry;
use crate::wit::papokin::plugin::player::Player;
use crate::wit::papokin::plugin::world::LivingEntity;

fn living_of(player: &Player) -> Option<LivingEntity> {
    player.as_entity().as_living()
}

/// [`Player`] 句柄的战斗追踪查询。
///
/// 玩家是生物实体；这些辅助方法转发到 `living-entity`
/// 战斗追踪器方法，使插件无需手动转换句柄。
pub trait PlayerCombatExt {
    /// 返回当前战斗回合中记录的伤害条目，
    /// 最旧的排在最前。玩家近期未受伤时为空（
    /// 追踪器在无伤害超时后重置）。
    fn get_combat_entries(&self) -> Vec<CombatEntry>;

    ///返回被记为击杀者的攻击者的伤害条目：
    /// 伤害最高的存活攻击者，满足条件时优先选择玩家攻击者
    /// 玩家的伤害满足原版的三分之一规则。若没有
    /// 存活攻击者会被记录。
    fn get_killer(&self) -> Option<CombatEntry>;

    /// 返回玩家当前是否被标记为处于战斗状态。
    fn is_in_combat(&self) -> bool;

    ///返回以毫秒为单位的战斗持续时间：战斗结束减去战斗开始
    /// 时刻，或仍在战斗中时为当前时间减去战斗开始时刻。
    fn get_combat_duration_ms(&self) -> i64;

    /// 返回最后一次确认命中的伤害类型名称：该
    /// 原版消息 ID（例如 `"arrow"`），或某项的命名空间名称
    /// 插件注册的自定义伤害类型。未记住任何击中时为 `None`
    /// （40 刻后遗忘）。
    fn get_last_damage_type_name(&self) -> Option<String>;

    /// 返回当前战斗回合中的攻击者是否有玩家。
    fn has_player_attacker(&self) -> bool;
}

impl PlayerCombatExt for Player {
    fn get_combat_entries(&self) -> Vec<CombatEntry> {
        living_of(self).map_or_else(Vec::new, |living| living.get_combat_entries())
    }

    fn get_killer(&self) -> Option<CombatEntry> {
        living_of(self).and_then(|living| living.get_killer())
    }

    fn is_in_combat(&self) -> bool {
        living_of(self).is_some_and(|living| living.is_in_combat())
    }

    fn get_combat_duration_ms(&self) -> i64 {
        living_of(self).map_or(0, |living| living.get_combat_duration_ms())
    }

    fn get_last_damage_type_name(&self) -> Option<String> {
        living_of(self).and_then(|living| living.get_last_damage_type_name())
    }

    fn has_player_attacker(&self) -> bool {
        living_of(self).is_some_and(|living| living.has_player_attacker())
    }
}
