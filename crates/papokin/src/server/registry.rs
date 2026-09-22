//! 插件可写注册表：为同步注册表提供自定义条目。
//!
//! 插件通过相应管理器注册自定义条目（例如新的伤害类型），
//! [`RegistryManager`] 中（服务器启动期间）。当客户端
//! 连接时，这些条目会被追加到注册表数据包中发送的
//! 原版同步注册表，因此自定义条目对给定
//! 协议版本而言，其网络 id 就是该版本的原版条目数加上该条目的
//! 注册索引。注册会在 [`RegistryManager::freeze`]
//! 会在插件加载完成后被调用一次。
//!
//! 只有通过 `CRegistryData` 同步注册表的协议版本
//! 数据包（1.20.2+）以这种方式接收自定义条目；较旧的客户端则使用
//! 登录注册表编解码器（`build_v1_20_registry_codec`），它不会随之扩展
//! 在此处。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{PoisonError, RwLock};

use papokin_data::registry::{Registry, RegistryEntryData};
use papokin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashMap;

/// 一个插件注册的自定义注册表条目：其带命名空间的 id 以及
/// 序列化后的 NBT 载荷，随注册表数据包发送给客户端。
#[derive(Clone, Debug)]
pub struct CustomRegistryEntry {
    pub name: String,
    // 例如 "myplugin:frost"
    pub nbt: Vec<u8>,
    // 序列化后的 NBT 复合标签（网络格式）
}

/// 保存插件为同步注册表注册的自定义条目，并移交
/// 分配网络 id。
///
/// 条目保持注册顺序；自定义条目的
/// 给定协议版本的网络 id 即该版本原版条目的
/// 条目数加上条目索引。在 `freeze` 之前允许注册
/// 在插件加载完成时翻转。
pub struct RegistryManager {
    /// 每个领域的自定义条目（例如 "`damage_type`"），按注册顺序排列。
    custom: RwLock<FxHashMap<String, Vec<CustomRegistryEntry>>>,
    /// 在插件加载完成后置位；后续注册将失败。
    frozen: AtomicBool,
    /// 各域的原版条目数，按协议版本缓存（键为
    /// `JavaMinecraftVersion::protocol_version`），因此网络 id 查找不会
    /// 在每次调用时重新克隆同步注册表。
    vanilla_counts: RwLock<FxHashMap<i32, FxHashMap<String, u16>>>,
}

impl RegistryManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            custom: RwLock::new(FxHashMap::default()),
            frozen: AtomicBool::new(false),
            vanilla_counts: RwLock::new(FxHashMap::default()),
        }
    }

    /// 在 `domain` 下注册自定义条目（例如 "`damage_type`"，不带
    /// "minecraft:" 前缀）。返回该条目在其域内的索引。
    /// 在注册表已冻结或名称与现有自定义条目重复时报错。
    pub fn register(&self, domain: &str, name: String, nbt: Vec<u8>) -> Result<u16, String> {
        let mut custom = self.custom.write().unwrap_or_else(PoisonError::into_inner);
        if self.frozen.load(Ordering::Acquire) {
            return Err(format!(
                "Cannot register registry entry '{name}' in domain '{domain}': \
                 the registry is frozen"
            ));
        }
        let entries = custom.entry(domain.to_string()).or_default();
        if entries.iter().any(|entry| entry.name == name) {
            return Err(format!(
                "Custom registry entry '{name}' is already registered in domain '{domain}'"
            ));
        }
        let index = u16::try_from(entries.len())
            .map_err(|_| format!("Registry domain '{domain}' is full"))?;
        entries.push(CustomRegistryEntry { name, nbt });
        Ok(index)
    }

    pub fn freeze(&self) {
        self.frozen.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::Acquire)
    }

    /// 一个域的所有自定义条目，按注册顺序排列。
    #[must_use]
    pub fn entries_for(&self, domain: &str) -> Vec<CustomRegistryEntry> {
        self.custom
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(domain)
            .cloned()
            .unwrap_or_default()
    }

    /// 按名称取自定义条目的索引（其在原版条目之后的偏移量）。
    #[must_use]
    pub fn index_of(&self, domain: &str, name: &str) -> Option<u16> {
        let custom = self.custom.read().unwrap_or_else(PoisonError::into_inner);
        custom
            .get(domain)?
            .iter()
            .position(|entry| entry.name == name)
            .and_then(|index| u16::try_from(index).ok())
    }

    /// `index` 处自定义条目的名称（如果存在）。
    #[must_use]
    pub fn name_at(&self, domain: &str, index: u16) -> Option<String> {
        let custom = self.custom.read().unwrap_or_else(PoisonError::into_inner);
        custom
            .get(domain)?
            .get(usize::from(index))
            .map(|entry| entry.name.clone())
    }

    /// 自定义条目在某个客户端协议版本下的网络 id：该版本的
    /// 原版条目数加上自定义索引。对未知的
    /// 域或名称。原版数量从已同步的静态表中查询。
    /// 表通过 `Registry::get_synced(version)` 获取并按版本缓存。
    #[must_use]
    pub fn custom_network_id(
        &self,
        domain: &str,
        name: &str,
        version: JavaMinecraftVersion,
    ) -> Option<u16> {
        let index = self.index_of(domain, name)?;
        let vanilla_count = self.vanilla_count(domain, version)?;
        u16::try_from(u32::from(vanilla_count) + u32::from(index)).ok()
    }

    /// `version` 下 `domain` 的原版条目数，来自同步的静态
    /// 表。版本中每个域的条目数都在首次使用时缓存。
    fn vanilla_count(&self, domain: &str, version: JavaMinecraftVersion) -> Option<u16> {
        let key = version.protocol_version();
        {
            let cache = self
                .vanilla_counts
                .read()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(counts) = cache.get(&key) {
                return counts.get(domain).copied();
            }
        }

        let mut counts = FxHashMap::default();
        for reg in Registry::get_synced(version) {
            counts.insert(
                domain_of(&reg.registry_id).to_string(),
                u16::try_from(reg.registry_entries.len()).unwrap_or(u16::MAX),
            );
        }
        let count = counts.get(domain).copied();
        // 竞争中的线程可能已缓存同一版本；静态
        // 表无论哪种方式都相同，因此覆盖是无害的。
        self.vanilla_counts
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, counts);
        count
    }
}

impl Default for RegistryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 同步注册表 ID 的注册表域：即去掉其
/// "minecraft:" 前缀（例如 "`damage_type`" 即 "`minecraft:damage_type`"）。
fn domain_of(registry_id: &str) -> &str {
    registry_id
        .strip_prefix("minecraft:")
        .unwrap_or(registry_id)
}

/// 将插件注册的每个自定义条目追加到其同步的注册表中。
///
/// 条目按注册顺序追加，因此它们会被发送给
/// 客户端则紧随原版条目之后。没有自定义条目的注册表
/// 保持原样。
pub fn inject_custom_entries(registries: &mut [Registry], manager: &RegistryManager) {
    for reg in registries {
        let custom_entries = manager.entries_for(domain_of(&reg.registry_id));
        reg.registry_entries
            .extend(custom_entries.into_iter().map(|entry| RegistryEntryData {
                entry_id: entry.name,
                data: Some(entry.nbt.into_boxed_slice()),
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_returns_sequential_indices() {
        let manager = RegistryManager::new();
        assert_eq!(
            manager.register("damage_type", "myplugin:frost".to_string(), vec![1]),
            Ok(0)
        );
        assert_eq!(
            manager.register("damage_type", "myplugin:burn".to_string(), vec![2]),
            Ok(1)
        );
    }

    #[test]
    fn register_after_freeze_is_rejected() {
        let manager = RegistryManager::new();
        assert!(!manager.is_frozen());
        manager.freeze();
        assert!(manager.is_frozen());
        assert!(
            manager
                .register("damage_type", "myplugin:frost".to_string(), vec![])
                .is_err()
        );
    }

    #[test]
    fn duplicate_name_is_rejected() {
        let manager = RegistryManager::new();
        manager
            .register("damage_type", "myplugin:frost".to_string(), vec![])
            .unwrap();
        assert!(
            manager
                .register("damage_type", "myplugin:frost".to_string(), vec![])
                .is_err()
        );
    }

    #[test]
    fn index_of_and_name_at_roundtrip() {
        let manager = RegistryManager::new();
        manager
            .register("damage_type", "myplugin:frost".to_string(), vec![1, 2])
            .unwrap();
        manager
            .register("damage_type", "myplugin:burn".to_string(), vec![3])
            .unwrap();

        assert_eq!(manager.index_of("damage_type", "myplugin:frost"), Some(0));
        assert_eq!(manager.index_of("damage_type", "myplugin:burn"), Some(1));
        assert_eq!(manager.index_of("damage_type", "myplugin:missing"), None);
        assert_eq!(
            manager.name_at("damage_type", 0),
            Some("myplugin:frost".to_string())
        );
        assert_eq!(
            manager.name_at("damage_type", 1),
            Some("myplugin:burn".to_string())
        );
        assert_eq!(manager.name_at("damage_type", 2), None);

        let entries = manager.entries_for("damage_type");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "myplugin:frost");
        assert_eq!(entries[0].nbt, vec![1, 2]);
        assert_eq!(entries[1].name, "myplugin:burn");
    }

    #[test]
    fn domains_are_isolated() {
        let manager = RegistryManager::new();
        manager
            .register("damage_type", "myplugin:frost".to_string(), vec![])
            .unwrap();
        // 不同域中的相同名称是不同的条目。
        assert_eq!(
            manager.register("chat_type", "myplugin:frost".to_string(), vec![]),
            Ok(0)
        );

        assert_eq!(manager.entries_for("damage_type").len(), 1);
        assert_eq!(manager.entries_for("chat_type").len(), 1);
        assert_eq!(manager.index_of("damage_type", "myplugin:frost"), Some(0));
        assert_eq!(manager.index_of("chat_type", "myplugin:frost"), Some(0));
        assert!(manager.entries_for("worldgen/biome").is_empty());
        assert_eq!(manager.index_of("worldgen/biome", "myplugin:frost"), None);
        assert_eq!(manager.name_at("worldgen/biome", 0), None);
    }

    #[test]
    fn custom_network_id_adds_vanilla_count() {
        let manager = RegistryManager::new();
        manager
            .register("damage_type", "myplugin:frost".to_string(), vec![])
            .unwrap();
        manager
            .register("damage_type", "myplugin:burn".to_string(), vec![])
            .unwrap();

        let vanilla_count = Registry::get_synced(JavaMinecraftVersion::V_1_21)
            .iter()
            .find(|reg| reg.registry_id == "minecraft:damage_type")
            .map(|reg| reg.registry_entries.len())
            .expect("1.21 数据表包含 damage_type 注册表");
        let vanilla_count = u16::try_from(vanilla_count).unwrap();

        assert_eq!(
            manager.custom_network_id(
                "damage_type",
                "myplugin:frost",
                JavaMinecraftVersion::V_1_21
            ),
            Some(vanilla_count)
        );
        assert_eq!(
            manager.custom_network_id("damage_type", "myplugin:burn", JavaMinecraftVersion::V_1_21),
            Some(vanilla_count + 1)
        );
        // 未知的名称和域不会得到 id。
        assert_eq!(
            manager.custom_network_id(
                "damage_type",
                "myplugin:missing",
                JavaMinecraftVersion::V_1_21
            ),
            None
        );
        assert_eq!(
            manager.custom_network_id(
                "not_a_registry",
                "myplugin:frost",
                JavaMinecraftVersion::V_1_21
            ),
            None
        );

        // 第二个版本会走按版本缓存；1.20 表
        // 与 1.21 不同，这也证明这些版本并不互为别名。
        let vanilla_1_20 = Registry::get_synced(JavaMinecraftVersion::V_1_20)
            .iter()
            .find(|reg| reg.registry_id == "minecraft:damage_type")
            .map(|reg| reg.registry_entries.len())
            .expect("1.20 数据表包含 damage_type 注册表");
        assert_eq!(
            manager.custom_network_id(
                "damage_type",
                "myplugin:frost",
                JavaMinecraftVersion::V_1_20
            ),
            u16::try_from(vanilla_1_20).ok()
        );
    }

    #[test]
    fn inject_appends_custom_entries_in_order() {
        let manager = RegistryManager::new();
        manager
            .register("damage_type", "myplugin:frost".to_string(), vec![0xA])
            .unwrap();
        manager
            .register("damage_type", "myplugin:burn".to_string(), vec![0xB])
            .unwrap();

        let mut registries = Registry::get_synced(JavaMinecraftVersion::V_1_21);
        let vanilla_count = registries
            .iter()
            .find(|reg| reg.registry_id == "minecraft:damage_type")
            .unwrap()
            .registry_entries
            .len();
        let biome_count = registries
            .iter()
            .find(|reg| reg.registry_id == "minecraft:worldgen/biome")
            .unwrap()
            .registry_entries
            .len();

        inject_custom_entries(&mut registries, &manager);

        let damage_type = registries
            .iter()
            .find(|reg| reg.registry_id == "minecraft:damage_type")
            .unwrap();
        assert_eq!(damage_type.registry_entries.len(), vanilla_count + 2);
        let frost = &damage_type.registry_entries[vanilla_count];
        assert_eq!(frost.entry_id, "myplugin:frost");
        assert_eq!(frost.data.as_deref(), Some(&[0xA][..]));
        let burn = &damage_type.registry_entries[vanilla_count + 1];
        assert_eq!(burn.entry_id, "myplugin:burn");
        assert_eq!(burn.data.as_deref(), Some(&[0xB][..]));

        // 没有自定义条目的域名不受影响。
        let biome = registries
            .iter()
            .find(|reg| reg.registry_id == "minecraft:worldgen/biome")
            .unwrap();
        assert_eq!(biome.registry_entries.len(), biome_count);
    }
}
