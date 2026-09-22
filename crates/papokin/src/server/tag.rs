//! 插件可写标签覆盖层：对静态标签表的运行时增补。
//!
//! 标签以静态的按版本表形式编译进 `papokin-data`
//! （`papokin_data::tag`）。服务器运行期间插件使用 [`TagManager`]，
//! 启动期间向现有标签追加条目、从中移除条目，
//! 或创建全新的标签。客户端连接时，覆盖层会与
//! 静态表合并，结果会通过 update-tags 数据包发送，
//! （配置阶段发送 `CUpdateTags`，pre-1.20.2 客户端版本发送 `CUpdateTagsPlay`
//! 处于游戏状态的客户端），因此模组客户端看到的插件条目与
//! 原版条目完全一样。
//!
//! 在网络上，标签是注册表 id 的列表，而插件 API 则按名称操作
//! 配合资源名称（`"minecraft:stone"`、`"myplugin:frost"`）使用。名称
//! 在为客户端协议版本构建快照时解析：原版
//! 名称通过内置的静态注册表解析（并且按版本
//! 重映射的方式与静态表 id 完全相同），而自定义名称则
//! 通过 [`RegistryManager::custom_network_id`] 解析，即
//! 注册表同步为该条目分配的网络 id（`原版数量 + 注册
//! 索引`）。因此注册表数据包始终与
//! 标签数据包所引用的 id 一致。
//!
//! 插件注册完成后，调用 [`TagManager::freeze`] 即会关闭注册，
//! 于插件加载完成后被调用时关闭，与 [`RegistryManager`] 一致。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

use papokin_data::tag::{RegistryKey, Tag, get_registry_key_tags};
use papokin_protocol::tag_overlay::MergedTags;
use papokin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashMap;

use super::registry::RegistryManager;

/// 一个标签的覆盖层：插件向该标签追加和移除的条目，
/// 按应用顺序排列。两个集合保持互斥（最后一个操作
/// 以名称为准者胜出）。
#[derive(Clone, Debug, Default)]
struct TagOverlay {
    /// 插件追加的条目名称（原版 `"minecraft:..."` 名称，或
    /// 自定义命名空间 id），去重后按插入顺序返回。
    added: Vec<String>,
    /// 插件移除的条目名称（来自静态表或来自 `added`）。
    removed: Vec<String>,
}

impl TagOverlay {
    fn is_removed(&self, name: &str) -> bool {
        self.removed.iter().any(|entry| names_equal(entry, name))
    }

    fn contains_added(&self, name: &str) -> bool {
        self.added.iter().any(|entry| names_equal(entry, name))
    }
}

/// 无论是否带显式 `minecraft:` 前缀，两个条目名称都视为相等
/// (`"stone"` 与 `"minecraft:stone"` 指向同一个原版条目)。自定义
/// 命名空间按原样逐字比较。
fn names_equal(a: &str, b: &str) -> bool {
    let a = a.strip_prefix("minecraft:").unwrap_or(a);
    let b = b.strip_prefix("minecraft:").unwrap_or(b);
    a == b
}

/// 保存插件对静态标签表的修改，并将其合并到
/// 发送给正在连接的客户端的标签同步数据包。
///
/// 在 [`TagManager::freeze`] 因插件而翻转之前允许修改
/// 加载完成时（即 [`RegistryManager`] 冻结的同一时点）。
pub struct TagManager {
    /// 按注册表键和标签名组织的覆盖层。
    overlay: RwLock<FxHashMap<RegistryKey, FxHashMap<String, TagOverlay>>>,
    /// 在插件加载完成后置位；后续修改将失败。
    frozen: AtomicBool,
    /// 自定义注册表条目，用于将自定义标签条目解析到
    /// 网络 id 由注册表同步为它们分配。
    registry_manager: Arc<RegistryManager>,
}

impl TagManager {
    #[must_use]
    pub fn new(registry_manager: Arc<RegistryManager>) -> Self {
        Self {
            overlay: RwLock::new(FxHashMap::default()),
            frozen: AtomicBool::new(false),
            registry_manager,
        }
    }

    /// 将 `entry_name` 追加到 `registry_key` 的标签 `tag_name` 中，必要时会创建
    /// 该标签既不在静态表中也不在
    /// 覆盖层。`entry_name` 是资源名称：原版名称
    /// (`"minecraft:stone"` 或不带前缀的 `"stone"`)，或是自定义命名空间 id
    /// (`"myplugin:frost"`)，且已在 [`RegistryManager`] 中注册。
    ///
    /// 添加标签中已存在的名称是无操作。而添加名称
    /// 曾被移除的插件会重新添加它（以最后一次操作为准）。
    /// 在标签表已冻结时报错。
    pub fn add_to_tag(
        &self,
        registry_key: RegistryKey,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), String> {
        let mut overlay = self.overlay.write().unwrap_or_else(PoisonError::into_inner);
        if self.frozen.load(Ordering::Acquire) {
            return Err(format!(
                "Cannot add '{entry_name}' to tag '{tag_name}': the tag tables are frozen"
            ));
        }
        let tag = overlay
            .entry(registry_key)
            .or_default()
            .entry(tag_name.to_string())
            .or_default();
        // 对同一名称的最后一次操作生效。
        tag.removed.retain(|entry| !names_equal(entry, entry_name));
        if !tag.contains_added(entry_name) {
            tag.added.push(entry_name.to_string());
        }
        Ok(())
    }

    /// 从 `registry_key` 的标签 `tag_name` 中移除 `entry_name`。该
    /// 条目可能来自静态表，或来自之前的
    /// [`TagManager::add_to_tag`]；移除一个不在标签中的名称
    /// 仍会被记录，因此之后重新添加静态表时仍保持移除状态。
    /// 在标签表已冻结时报错。
    pub fn remove_from_tag(
        &self,
        registry_key: RegistryKey,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), String> {
        let mut overlay = self.overlay.write().unwrap_or_else(PoisonError::into_inner);
        if self.frozen.load(Ordering::Acquire) {
            return Err(format!(
                "Cannot remove '{entry_name}' from tag '{tag_name}': the tag tables are frozen"
            ));
        }
        let tag = overlay
            .entry(registry_key)
            .or_default()
            .entry(tag_name.to_string())
            .or_default();
        // 对同一名称的最后一次操作生效。
        tag.added.retain(|entry| !names_equal(entry, entry_name));
        if !tag.is_removed(entry_name) {
            tag.removed.push(entry_name.to_string());
        }
        Ok(())
    }

    /// 创建一个初始为空的新标签。[`TagManager::add_to_tag`] 会创建
    /// 标签；显式创建适合用于发布空标签。
    /// 在标签表已冻结时报错。
    pub fn create_tag(&self, registry_key: RegistryKey, tag_name: &str) -> Result<(), String> {
        let mut overlay = self.overlay.write().unwrap_or_else(PoisonError::into_inner);
        if self.frozen.load(Ordering::Acquire) {
            return Err(format!(
                "Cannot create tag '{tag_name}': the tag tables are frozen"
            ));
        }
        overlay
            .entry(registry_key)
            .or_default()
            .entry(tag_name.to_string())
            .or_default();
        Ok(())
    }

    pub fn freeze(&self) {
        self.frozen.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::Acquire)
    }

    /// 叠加层是否触及 `registry_key`。
    #[must_use]
    pub fn has_overlay_for(&self, registry_key: RegistryKey) -> bool {
        self.overlay
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&registry_key)
            .is_some_and(|tags| !tags.is_empty())
    }

    /// 单个标签条目名称的合并视图：静态表（最新版本，
    /// 内置版本）加上覆盖层新增项，减去覆盖层移除项。
    /// 名称保持其静态表形式（裸路径）；覆盖层新增的条目
    /// 保留插件所用的形式。当静态表和
    /// 还是覆盖层都不知道该标签。
    #[must_use]
    pub fn get_tag_values(&self, registry_key: RegistryKey, tag_name: &str) -> Vec<String> {
        let static_names = papokin_data::tag::get_tag_values(registry_key, tag_name);
        let overlay = self.overlay.read().unwrap_or_else(PoisonError::into_inner);
        let tag_overlay = overlay
            .get(&registry_key)
            .and_then(|tags| tags.get(tag_name));

        let mut values: Vec<String> = Vec::new();
        if let Some(names) = static_names {
            values.extend(
                names
                    .iter()
                    .filter(|name| !tag_overlay.is_some_and(|ov| ov.is_removed(name)))
                    .map(|name| (*name).to_string()),
            );
        }
        if let Some(ov) = tag_overlay {
            for entry in &ov.added {
                if !values.iter().any(|value| names_equal(value, entry)) {
                    values.push(entry.clone());
                }
            }
        }
        values
    }

    /// 其标签必须发送给 `version` 客户端的注册表键：
    /// 每个具有非空静态表的网络同步键，以及每个
    /// 覆盖层触及的范围。没有覆盖层时，这恰好就是
    /// 服务器在标签变为可由插件写入之前所发送的内容。
    #[must_use]
    pub fn network_tag_keys(&self, version: JavaMinecraftVersion) -> Vec<RegistryKey> {
        let overlay = self.overlay.read().unwrap_or_else(PoisonError::into_inner);
        RegistryKey::NETWORK_KEYS
            .iter()
            .copied()
            .filter(|&key| {
                get_registry_key_tags(version, key).is_some_and(|map| !map.is_empty())
                    || (key.is_valid_for_version(version)
                        && overlay.get(&key).is_some_and(|tags| !tags.is_empty()))
            })
            .collect()
    }

    /// 构建发送给 `version` 客户端的合并标签映射：对于每个
    /// 覆盖层涉及的每个注册表键，静态表（对应版本的）
    /// 加上覆盖层新增项、减去覆盖层移除项，其中每个条目
    /// 解析为其最终的网络传输 id。
    ///
    /// ID 解析使标签数据包与该注册表数据保持一致
    /// 客户端最先收到的：静态条目与原版新增项解析为
    /// 它们的内置注册表 ID，并会针对客户端版本进行重映射
    /// 与静态路径完全相同；自定义新增项通过
    /// [`RegistryManager::custom_network_id`]（该版本的原版条目
    /// 条目数加上条目的注册索引），并按原样写入。
    ///
    ///当覆盖层为空时返回 `None`，让调用者回退到默认行为
    /// 未修改的静态表序列化。
    #[must_use]
    pub fn snapshot(&self, version: JavaMinecraftVersion) -> Option<MergedTags> {
        let overlay = self.overlay.read().unwrap_or_else(PoisonError::into_inner);
        if overlay.is_empty() {
            return None;
        }

        let mut maps = HashMap::new();
        for (&key, key_overlay) in overlay.iter() {
            if !key.is_valid_for_version(version) {
                continue;
            }
            let static_map = get_registry_key_tags(version, key);
            let mut entries: Vec<(String, Vec<u16>)> = Vec::new();

            if let Some(map) = static_map {
                for (tag_name, tag) in map.entries() {
                    let tag_overlay = key_overlay.get(*tag_name);
                    let ids = self.merged_static_tag_ids(key, tag, tag_overlay, version);
                    entries.push(((*tag_name).to_string(), ids));
                }
            }
            for (tag_name, tag_overlay) in key_overlay {
                if static_map.is_some_and(|map| map.contains_key(tag_name.as_str())) {
                    continue;
                }
                let ids = self.merged_overlay_tag_ids(key, tag_overlay, version);
                entries.push((tag_name.clone(), ids));
            }

            maps.insert(key, entries);
        }

        if maps.is_empty() {
            None
        } else {
            Some(MergedTags { maps })
        }
    }

    /// 将一个静态标签与其覆盖层合并：静态 ID（经过重新映射，
    /// 客户端版本）减去移除项，再加上已解析的新增项。
    fn merged_static_tag_ids(
        &self,
        key: RegistryKey,
        tag: &Tag,
        tag_overlay: Option<&TagOverlay>,
        version: JavaMinecraftVersion,
    ) -> Vec<u16> {
        let mut ids = Vec::with_capacity(tag.1.len());
        for (name, id) in tag.0.iter().zip(tag.1.iter()) {
            if tag_overlay.is_some_and(|ov| ov.is_removed(name)) {
                continue;
            }
            ids.push(remap_tag_entry_id(key, *id, version));
        }
        if let Some(ov) = tag_overlay {
            for entry in &ov.added {
                // 已位于静态标签中的名称从未真正被添加。
                if tag.0.iter().any(|name| names_equal(name, entry)) {
                    continue;
                }
                self.push_resolved(&mut ids, key, entry, version);
            }
        }
        ids
    }

    /// 解析仅存在于覆盖层（新增）中的标签的每个条目。
    fn merged_overlay_tag_ids(
        &self,
        key: RegistryKey,
        tag_overlay: &TagOverlay,
        version: JavaMinecraftVersion,
    ) -> Vec<u16> {
        let mut ids = Vec::with_capacity(tag_overlay.added.len());
        for entry in &tag_overlay.added {
            self.push_resolved(&mut ids, key, entry, version);
        }
        ids
    }

    /// 为 `version` 解析 `entry`，并将其最终的网络传输 id 压入
    /// `ids`，对内置注册表的 id 应用版本重映射，并将
    /// 自定义注册表 id 透传。未知条目会被跳过并
    /// 警告，这样单个有问题的插件条目就不会破坏整个标签数据包。
    fn push_resolved(
        &self,
        ids: &mut Vec<u16>,
        key: RegistryKey,
        entry: &str,
        version: JavaMinecraftVersion,
    ) {
        if let Some((id, needs_remap)) = self.resolve_entry(key, entry, version) {
            ids.push(if needs_remap {
                remap_tag_entry_id(key, id, version)
            } else {
                id
            });
        } else {
            tracing::warn!(
                "标签叠加：未知的 {} 条目 '{entry}'；已跳过",
                key.identifier_string(),
            );
        }
    }

    /// 为 `version` 将条目名称解析为其网络 id。该布尔值
    /// 当该 ID 是仍需版本映射的内置注册表 ID 时为 `true`
    /// 重映射，当它是按版本的最终 id 时返回 `false`（自定义
    /// 注册表条目，注册表同步将其放在原版
    /// 条目，以及哪些必须原样透传）。
    fn resolve_entry(
        &self,
        key: RegistryKey,
        entry: &str,
        version: JavaMinecraftVersion,
    ) -> Option<(u16, bool)> {
        let (is_custom_namespace, path) = match entry.split_once(':') {
            Some((namespace, path)) => (namespace != "minecraft", path),
            None => (false, entry),
        };
        if is_custom_namespace {
            return self
                .registry_manager
                .custom_network_id(key.identifier_string(), entry, version)
                .map(|id| (id, false));
        }
        vanilla_entry_id(key, path).map(|id| (id, true))
    }
}

/// 最新内置注册表（`REGISTRY_V_26_3`）中原版条目的 ID
/// 适用于同步注册表（硬编码的则用静态类）——相同的
/// 静态标签表所用的 id 基准，因此调用方的版本重映射
/// 对它的处理方式与对静态表 ID 完全相同。
///
/// `point_of_interest_type` 没有内置名称表，因此原版新增的
/// 其标签无法解析（它们会伴随警告被跳过）；自定义
/// 新增内容仍能正常工作。
fn vanilla_entry_id(key: RegistryKey, path: &str) -> Option<u16> {
    match key {
        RegistryKey::Block => {
            papokin_data::Block::from_registry_key(path).map(|block| block.id.as_u16())
        }
        RegistryKey::Item => papokin_data::item::Item::from_registry_key(path).map(|item| item.id),
        RegistryKey::Fluid => {
            papokin_data::fluid::Fluid::from_registry_key(path).map(|fluid| fluid.id)
        }
        RegistryKey::EntityType => {
            papokin_data::entity::EntityType::from_name(path).map(|entity| entity.id)
        }
        RegistryKey::GameEvent => {
            papokin_data::game_event::GameEvent::from_name(path).map(|event| event as u16)
        }
        RegistryKey::Potion => {
            papokin_data::potion::Potion::from_name(path).map(|potion| u16::from(potion.id))
        }
        _ => papokin_data::registry::REGISTRY_V_26_3
            .iter()
            .find(|registry| registry.registry_id == key.identifier_string())
            .and_then(|registry| registry.entries.iter().position(|entry| entry.name == path))
            .and_then(|index| u16::try_from(index).ok()),
    }
}

/// 应用于内置注册表 id 的版本重映射，与
/// 标签包内的静态路径重映射：物品与实体类型 id 会
/// 跨版本重映射，其余所有注册表的 id 原样透传。
fn remap_tag_entry_id(key: RegistryKey, id: u16, version: JavaMinecraftVersion) -> u16 {
    match key {
        RegistryKey::Item => papokin_data::item_id_remap::remap_item_id_for_version(id, version),
        RegistryKey::EntityType => {
            papokin_data::entity_id_remap::remap_entity_id_for_version(id, version)
        }
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> (Arc<RegistryManager>, TagManager) {
        let registry_manager = Arc::new(RegistryManager::new());
        let tag_manager = TagManager::new(Arc::clone(&registry_manager));
        (registry_manager, tag_manager)
    }

    #[test]
    fn add_creates_new_tag() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "myplugin:things", "minecraft:stone")
            .unwrap();
        assert_eq!(
            tags.get_tag_values(RegistryKey::Item, "myplugin:things"),
            vec!["minecraft:stone".to_string()]
        );
        assert!(tags.has_overlay_for(RegistryKey::Item));
        assert!(!tags.has_overlay_for(RegistryKey::Block));
    }

    #[test]
    fn add_to_static_tag_merges_names() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:stone")
            .unwrap();
        let values = tags.get_tag_values(RegistryKey::Item, "minecraft:anvil");
        assert!(values.iter().any(|name| name == "anvil"));
        assert!(values.iter().any(|name| name == "minecraft:stone"));
        // 静态条目在前，新增条目在后。
        assert_eq!(values.last().unwrap(), "minecraft:stone");
    }

    #[test]
    fn bare_and_namespaced_names_are_the_same_entry() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "stone")
            .unwrap();
        // 使用显式命名空间添加相同条目是无操作。
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:stone")
            .unwrap();
        let values = tags.get_tag_values(RegistryKey::Item, "minecraft:anvil");
        assert_eq!(values.iter().filter(|name| *name == "stone").count(), 1);
    }

    #[test]
    fn remove_deletes_static_entry() {
        let (_registries, tags) = manager();
        tags.remove_from_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:anvil")
            .unwrap();
        let values = tags.get_tag_values(RegistryKey::Item, "minecraft:anvil");
        assert!(!values.iter().any(|name| name == "anvil"));
        assert!(values.iter().any(|name| name == "chipped_anvil"));
    }

    #[test]
    fn last_operation_on_a_name_wins() {
        let (_registries, tags) = manager();
        // 添加 -> 移除 -> 消失
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:stone")
            .unwrap();
        tags.remove_from_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:stone")
            .unwrap();
        assert!(
            !tags
                .get_tag_values(RegistryKey::Item, "minecraft:anvil")
                .iter()
                .any(|name| name == "minecraft:stone")
        );
        // 移除(static) -> 添加 -> 还原
        tags.remove_from_tag(RegistryKey::Item, "minecraft:anvil", "anvil")
            .unwrap();
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "anvil")
            .unwrap();
        assert!(
            tags.get_tag_values(RegistryKey::Item, "minecraft:anvil")
                .iter()
                .any(|name| name == "anvil")
        );
    }

    #[test]
    fn frozen_manager_rejects_writes() {
        let (_registries, tags) = manager();
        assert!(!tags.is_frozen());
        tags.freeze();
        assert!(tags.is_frozen());
        assert!(
            tags.add_to_tag(RegistryKey::Item, "myplugin:t", "minecraft:stone")
                .is_err()
        );
        assert!(
            tags.remove_from_tag(RegistryKey::Item, "minecraft:anvil", "anvil")
                .is_err()
        );
        assert!(tags.create_tag(RegistryKey::Item, "myplugin:t").is_err());
    }

    #[test]
    fn create_tag_publishes_empty_tag() {
        let (_registries, tags) = manager();
        tags.create_tag(RegistryKey::Block, "myplugin:empty")
            .unwrap();
        assert_eq!(
            tags.get_tag_values(RegistryKey::Block, "myplugin:empty"),
            Vec::<String>::new()
        );
        let snapshot = tags.snapshot(JavaMinecraftVersion::V_1_21).unwrap();
        let entries = snapshot.get(RegistryKey::Block).unwrap();
        assert!(
            entries
                .iter()
                .any(|(name, ids)| name == "myplugin:empty" && ids.is_empty())
        );
    }

    #[test]
    fn snapshot_is_none_without_overlay() {
        let (_registries, tags) = manager();
        assert!(tags.snapshot(JavaMinecraftVersion::V_1_21).is_none());
    }

    #[test]
    fn snapshot_merges_static_tag_and_resolves_vanilla_names() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:stone")
            .unwrap();
        tags.remove_from_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:anvil")
            .unwrap();

        let version = JavaMinecraftVersion::V_26_3;
        let snapshot = tags.snapshot(version).unwrap();
        let entries = snapshot.get(RegistryKey::Item).unwrap();
        let (_, ids) = entries
            .iter()
            .find(|(name, _)| name == "minecraft:anvil")
            .unwrap();

        let anvil = papokin_data::item::Item::from_registry_key("anvil")
            .unwrap()
            .id;
        let chipped = papokin_data::item::Item::from_registry_key("chipped_anvil")
            .unwrap()
            .id;
        let stone = papokin_data::item::Item::from_registry_key("stone")
            .unwrap()
            .id;
        // 26.3 是捆绑版本：重映射为恒等映射。
        assert!(!ids.contains(&anvil));
        assert!(ids.contains(&chipped));
        assert_eq!(ids.last().unwrap(), &stone);
    }

    #[test]
    fn snapshot_does_not_touch_other_keys() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "myplugin:things", "minecraft:stone")
            .unwrap();
        let snapshot = tags.snapshot(JavaMinecraftVersion::V_1_21).unwrap();
        assert!(snapshot.get(RegistryKey::Item).is_some());
        assert!(!snapshot.contains_key(RegistryKey::Block));
    }

    #[test]
    fn snapshot_applies_item_id_remapping_for_old_versions() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "minecraft:stone")
            .unwrap();

        let version = JavaMinecraftVersion::V_1_20_2;
        let snapshot = tags.snapshot(version).unwrap();
        let entries = snapshot.get(RegistryKey::Item).unwrap();
        let (_, ids) = entries
            .iter()
            .find(|(name, _)| name == "minecraft:anvil")
            .unwrap();

        let stone_native = papokin_data::item::Item::from_registry_key("stone")
            .unwrap()
            .id;
        let stone_1_20_2 =
            papokin_data::item_id_remap::remap_item_id_for_version(stone_native, version);
        assert!(ids.contains(&stone_1_20_2));
        // 静态 ID 以相同方式重新映射。
        let anvil_native = papokin_data::item::Item::from_registry_key("anvil")
            .unwrap()
            .id;
        let anvil_1_20_2 =
            papokin_data::item_id_remap::remap_item_id_for_version(anvil_native, version);
        assert!(ids.contains(&anvil_1_20_2));
    }

    #[test]
    fn custom_entries_use_the_registry_network_id_verbatim() {
        let (registries, tags) = manager();
        registries
            .register("damage_type", "myplugin:frost".to_string(), vec![1])
            .unwrap();
        tags.add_to_tag(
            RegistryKey::DamageType,
            "myplugin:custom_damage",
            "myplugin:frost",
        )
        .unwrap();

        let version = JavaMinecraftVersion::V_1_21;
        let snapshot = tags.snapshot(version).unwrap();
        let entries = snapshot.get(RegistryKey::DamageType).unwrap();
        let (_, ids) = entries
            .iter()
            .find(|(name, _)| name == "myplugin:custom_damage")
            .unwrap();
        let network_id = registries
            .custom_network_id("damage_type", "myplugin:frost", version)
            .unwrap();
        assert_eq!(ids, &vec![network_id]);

        // 该 id 等于同步注册表的原版条目数加上
        // 注册索引，与 `inject_custom_entries` 保持一致。
        let vanilla_count = papokin_data::registry::Registry::get_synced(version)
            .iter()
            .find(|reg| reg.registry_id == "minecraft:damage_type")
            .unwrap()
            .registry_entries
            .len();
        assert_eq!(usize::from(network_id), vanilla_count);
    }

    #[test]
    fn custom_entries_in_unsynced_registries_are_skipped() {
        let (registries, tags) = manager();
        // `RegistryManager` 只为已同步的注册表发放网络 id
        // （item/block/... 为硬编码，从不通过其同步
        // `CRegistryData`），因此自定义物品名称没有网络 id，且
        // 快照会跳过它并发出警告，而不是凭空捏造一个。
        registries
            .register("item", "myplugin:wand".to_string(), vec![2])
            .unwrap();
        assert!(
            registries
                .custom_network_id("item", "myplugin:wand", JavaMinecraftVersion::V_1_21)
                .is_none()
        );
        tags.add_to_tag(RegistryKey::Item, "myplugin:tools", "myplugin:wand")
            .unwrap();
        tags.add_to_tag(RegistryKey::Item, "myplugin:tools", "minecraft:stone")
            .unwrap();

        let snapshot = tags.snapshot(JavaMinecraftVersion::V_1_21).unwrap();
        let entries = snapshot.get(RegistryKey::Item).unwrap();
        let (_, ids) = entries
            .iter()
            .find(|(name, _)| name == "myplugin:tools")
            .unwrap();
        let stone = papokin_data::item::Item::from_registry_key("stone")
            .unwrap()
            .id;
        assert_eq!(ids, &vec![stone]);
    }

    #[test]
    fn unknown_entries_are_skipped_in_snapshot() {
        let (_registries, tags) = manager();
        tags.add_to_tag(
            RegistryKey::Item,
            "myplugin:things",
            "minecraft:not_a_real_item_xyz",
        )
        .unwrap();
        tags.add_to_tag(RegistryKey::Item, "myplugin:things", "myplugin:missing")
            .unwrap();
        tags.add_to_tag(RegistryKey::Item, "myplugin:things", "minecraft:stone")
            .unwrap();
        let snapshot = tags.snapshot(JavaMinecraftVersion::V_1_21).unwrap();
        let entries = snapshot.get(RegistryKey::Item).unwrap();
        let (_, ids) = entries
            .iter()
            .find(|(name, _)| name == "myplugin:things")
            .unwrap();
        let stone = papokin_data::item::Item::from_registry_key("stone")
            .unwrap()
            .id;
        assert_eq!(ids, &vec![stone]);
    }

    #[test]
    fn network_tag_keys_adds_overlay_only_keys() {
        let (_registries, tags) = manager();
        let version = JavaMinecraftVersion::V_1_21;
        let baseline: Vec<RegistryKey> = RegistryKey::NETWORK_KEYS
            .iter()
            .copied()
            .filter(|&key| get_registry_key_tags(version, key).is_some_and(|map| !map.is_empty()))
            .collect();
        // 没有覆盖层时，辅助函数会复现覆盖前的键集合。
        assert_eq!(tags.network_tag_keys(version), baseline);

        // 对该版本网络有效但没有静态（条目）的键
        // 表（例如 1.21 的药水）在覆盖层就绪后仍会被发送
        // 触碰它。对该版本无效的键必须保持不发送，因此
        // 把搜索限制在有效的那些上。
        let absent = RegistryKey::NETWORK_KEYS
            .iter()
            .copied()
            .find(|key| !baseline.contains(key) && key.is_valid_for_version(version));
        if let Some(key) = absent {
            tags.add_to_tag(key, "myplugin:things", "minecraft:stone")
                .unwrap();
            let keys = tags.network_tag_keys(version);
            assert!(keys.contains(&key));
            assert_eq!(keys.len(), baseline.len() + 1);
        } else {
            tags.add_to_tag(RegistryKey::Item, "myplugin:things", "minecraft:stone")
                .unwrap();
            assert_eq!(tags.network_tag_keys(version), baseline);
        }
    }

    #[test]
    fn synced_registry_vanilla_names_resolve() {
        let (_registries, tags) = manager();
        // damage_type 是同步注册表，没有硬编码名称表。
        tags.add_to_tag(
            RegistryKey::DamageType,
            "minecraft:is_fire",
            "minecraft:arrow",
        )
        .unwrap();
        let snapshot = tags.snapshot(JavaMinecraftVersion::V_26_3).unwrap();
        let entries = snapshot.get(RegistryKey::DamageType).unwrap();
        let (_, ids) = entries
            .iter()
            .find(|(name, _)| name == "minecraft:is_fire")
            .unwrap();
        let arrow = papokin_data::registry::REGISTRY_V_26_3
            .iter()
            .find(|reg| reg.registry_id == "damage_type")
            .unwrap()
            .entries
            .iter()
            .position(|entry| entry.name == "arrow")
            .unwrap();
        assert!(ids.contains(&(arrow as u16)));
    }
}
