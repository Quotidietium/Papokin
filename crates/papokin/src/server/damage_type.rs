//! 插件提供的自定义伤害类型的运行时注册。
//!
//! 插件在服务器启动时通过 [`DamageTypeManager`] 注册新的伤害类型，
//! 服务器启动期间（在 [`crate::server::registry::RegistryManager`]
//! 于插件加载结束时冻结）。一次注册会做两件事：
//!
//! - 该条目会被追加到对应注册表的 `damage_type` 域，
//!   [`crate::server::registry::RegistryManager`]，因此会同步到客户端
//!   在原版条目之后（其针对客户端版本的网络 id 为该
//!   版本的原版条目数加上注册序号），
//! - 保留 [`CustomDamageType`] 描述符，使伤害执行路径
//!   可将名称解析回其字段（死亡消息 id、饥饿消耗、
//!   ……）通过 [`DamageTypeManager::resolve`] 解析。
//!
//! 原版伤害类型不受影响：它们仍会解析到静态
//! [`DamageType`] 表。

use std::sync::{PoisonError, RwLock};

use papokin_data::damage::{DamageEffects, DamageScaling, DamageType, DeathMessageType};
use papokin_data::damage_ext::{
    CustomDamageType, ResolvedDamageType, custom_damage_type_nbt, native_damage_type_count,
};
use rustc_hash::FxHashMap;

use crate::server::Server;

/// 自定义伤害类型所注册到的同步注册表域。
pub const DAMAGE_TYPE_DOMAIN: &str = "damage_type";

/// 自定义伤害类型的注册输入。各字段与原版的
/// `damage_type` 注册表条目结构。
#[derive(Clone, Debug)]
pub struct CustomDamageTypeDefinition {
    /// 条目的带命名空间 id，例如 "`myplugin:frost`"。
    pub name: String,
    /// 死亡消息 ID；死亡消息翻译为 `death.attack.<message_id>`。
    pub message_id: String,
    /// 伤害量随难度缩放的情况。
    pub scaling: DamageScaling,
    /// 施加给受此伤害玩家的饥饿消耗。
    pub exhaustion: f32,
    /// 可选的受伤音效/视觉效果覆盖。
    pub effects: Option<DamageEffects>,
    /// 死亡消息的构成方式。
    pub death_message_type: DeathMessageType,
}

/// 已注册自定义伤害类型的一致快照：按名称查找
/// 加上注册顺序，从而固定网络 id（原生原版
/// 条目数 + 注册索引），由注册表同步分配。
struct CustomDamageTypeState {
    by_name: FxHashMap<String, CustomDamageType>,
    order: Vec<String>,
}

/// 保存插件注册的自定义伤害类型，并解析伤害类型
/// 标识符（原版名称或自定义命名空间 id）映射到表示形式
/// 伤害执行路径会消费。
pub struct DamageTypeManager {
    custom: RwLock<CustomDamageTypeState>,
}

impl DamageTypeManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            custom: RwLock::new(CustomDamageTypeState {
                by_name: FxHashMap::default(),
                order: Vec::new(),
            }),
        }
    }

    /// 端到端注册自定义伤害类型：将其同步给客户端，通过
    /// `server.registry_manager`（追加在原版条目之后
    /// `damage_type` 注册表），并保留其描述符以便解析名称。
    /// 返回条目在原生数据集 ID 空间中的网络 ID（原生
    /// 原版数量 + 注册索引）。
    ///
    /// # Errors
    /// 名称重复、与原版名称冲突，或注册表已冻结
    /// (插件加载已完成) 的情况会报告为 `Err`。
    pub fn register(
        &self,
        server: &Server,
        definition: CustomDamageTypeDefinition,
    ) -> Result<u16, String> {
        let mut state = self.custom.write().unwrap_or_else(PoisonError::into_inner);
        if state.by_name.contains_key(&definition.name) {
            return Err(format!(
                "Damage type '{}' is already registered",
                definition.name
            ));
        }
        // minecraft 命名空间（或无任何命名空间）中的名称必须
        // 不得遮蔽原版伤害类型。
        let stripped = definition
            .name
            .strip_prefix("minecraft:")
            .unwrap_or(&definition.name);
        if !stripped.contains(':') && DamageType::from_name(stripped).is_some() {
            return Err(format!(
                "Damage type '{}' collides with a vanilla damage type",
                definition.name
            ));
        }
        if server.registry_manager.is_frozen() {
            return Err(format!(
                "Cannot register damage type '{}': the registry is frozen",
                definition.name
            ));
        }

        let nbt = custom_damage_type_nbt(
            &definition.message_id,
            definition.scaling,
            definition.exhaustion,
            definition.effects,
            definition.death_message_type,
        );
        let index =
            server
                .registry_manager
                .register(DAMAGE_TYPE_DOMAIN, definition.name.clone(), nbt)?;
        let network_id = native_damage_type_count() + index;

        let custom = CustomDamageType {
            name: definition.name.clone(),
            message_id: definition.message_id,
            scaling: definition.scaling,
            exhaustion: definition.exhaustion,
            effects: definition.effects,
            death_message_type: definition.death_message_type,
            network_id,
        };
        state.order.push(definition.name.clone());
        state.by_name.insert(definition.name, custom);
        Ok(network_id)
    }

    /// 将伤害类型标识符解析为伤害
    /// 执行路径所消费。原版名称（无论是否带有
    /// "minecraft:" 前缀，例如 "arrow" 或 "minecraft:arrow"）解析到
    /// 静态 [`DamageType`] 表；其他类型则在
    /// 已注册的自定义伤害类型，按其完整命名空间 id 查找。
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<ResolvedDamageType> {
        let stripped = name.strip_prefix("minecraft:").unwrap_or(name);
        if let Some(vanilla) = DamageType::from_name(stripped) {
            return Some(ResolvedDamageType::Vanilla(vanilla));
        }
        self.custom
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .by_name
            .get(name)
            .cloned()
            .map(ResolvedDamageType::Custom)
    }

    /// 具有此命名空间 ID 的已注册自定义伤害类型（如有）。
    #[must_use]
    pub fn get(&self, name: &str) -> Option<CustomDamageType> {
        self.custom
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .by_name
            .get(name)
            .cloned()
    }

    /// 所有已注册的自定义伤害类型，按注册（网络 ID）顺序排列。
    #[must_use]
    pub fn all(&self) -> Vec<CustomDamageType> {
        let state = self.custom.read().unwrap_or_else(PoisonError::into_inner);
        state
            .order
            .iter()
            .filter_map(|name| state.by_name.get(name).cloned())
            .collect()
    }
}

impl Default for DamageTypeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_accepts_vanilla_names_with_or_without_prefix() {
        let manager = DamageTypeManager::new();
        assert_eq!(
            manager.resolve("arrow"),
            Some(ResolvedDamageType::Vanilla(DamageType::ARROW))
        );
        assert_eq!(
            manager.resolve("minecraft:wither"),
            Some(ResolvedDamageType::Vanilla(DamageType::WITHER))
        );
        assert_eq!(manager.resolve("myplugin:frost"), None);
        assert_eq!(manager.resolve("minecraft:not_a_damage_type"), None);
    }
}
