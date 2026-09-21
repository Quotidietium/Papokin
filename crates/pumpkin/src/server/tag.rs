//! Plugin-writable tag overlay: runtime additions to the static tag tables.
//!
//! Tags are compiled into `pumpkin-data` as static, per-version tables
//! (`pumpkin_data::tag`). Plugins use the [`TagManager`] while the server is
//! starting up to append entries to existing tags, remove entries from them,
//! or create entirely new tags. When a client connects, the overlay is merged
//! with the static tables and the result is sent in the update-tags packets
//! (`CUpdateTags` during configuration, `CUpdateTagsPlay` for pre-1.20.2
//! clients in play state), so modded clients see plugin entries exactly like
//! vanilla ones.
//!
//! On the wire a tag is a list of registry ids, but the plugin API works
//! with resource names (`"minecraft:stone"`, `"myplugin:frost"`). Names are
//! resolved when a snapshot is built for a client protocol version: vanilla
//! names resolve through the bundled static registries (and are version
//! remapped exactly like static-table ids), while custom names resolve
//! through [`RegistryManager::custom_network_id`], i.e. the same network id
//! the registry sync assigned the entry (`vanilla count + registration
//! index`). The registry data packet therefore always agrees with the ids
//! referenced by the tag packet.
//!
//! Bedrock edition has no server-pushed tag registry in its protocol (its
//! `item_tag` recipe descriptor is unrelated), so the overlay only applies to
//! the Java tag-sync path; Bedrock logins never touch it.
//!
//! Registration closes when [`TagManager::freeze`] is called once plugin
//! loading has finished, mirroring [`RegistryManager`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

use pumpkin_data::tag::{RegistryKey, Tag, get_registry_key_tags};
use pumpkin_protocol::tag_overlay::MergedTags;
use pumpkin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashMap;

use super::registry::RegistryManager;

/// One tag's overlay: entries plugins appended to and removed from the tag,
/// in application order. The two sets are kept disjoint (the last operation
/// on a name wins).
#[derive(Clone, Debug, Default)]
struct TagOverlay {
    /// Entry names appended by plugins (vanilla `"minecraft:..."` names or
    /// custom namespaced ids), deduplicated, in insertion order.
    added: Vec<String>,
    /// Entry names plugins removed (from the static table or from `added`).
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

/// Two entry names are equal regardless of an explicit `minecraft:` prefix
/// (`"stone"` and `"minecraft:stone"` name the same vanilla entry). Custom
/// namespaces compare verbatim.
fn names_equal(a: &str, b: &str) -> bool {
    let a = a.strip_prefix("minecraft:").unwrap_or(a);
    let b = b.strip_prefix("minecraft:").unwrap_or(b);
    a == b
}

/// Holds plugin modifications to the static tag tables and merges them into
/// the tag-sync packets sent to connecting clients.
///
/// Modifications are allowed until [`TagManager::freeze`] flips when plugin
/// loading finishes (the same point [`RegistryManager`] freezes).
pub struct TagManager {
    /// Overlay per registry key and tag name.
    overlay: RwLock<FxHashMap<RegistryKey, FxHashMap<String, TagOverlay>>>,
    /// Set once plugin loading has finished; further modifications fail.
    frozen: AtomicBool,
    /// Custom registry entries, used to resolve custom tag entries to the
    /// network ids the registry sync assigned them.
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

    /// Appends `entry_name` to the tag `tag_name` of `registry_key`, creating
    /// the tag when it exists neither in the static tables nor in the
    /// overlay. `entry_name` is a resource name: a vanilla name
    /// (`"minecraft:stone"` or bare `"stone"`) or a custom namespaced id
    /// (`"myplugin:frost"`) registered with the [`RegistryManager`].
    ///
    /// Adding a name that is already in the tag is a no-op. Adding a name
    /// that was previously removed re-adds it (the last operation wins).
    /// Errors when the tag tables are frozen.
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
        // The last operation on a name wins.
        tag.removed.retain(|entry| !names_equal(entry, entry_name));
        if !tag.contains_added(entry_name) {
            tag.added.push(entry_name.to_string());
        }
        Ok(())
    }

    /// Removes `entry_name` from the tag `tag_name` of `registry_key`. The
    /// entry may come from the static tables or from a previous
    /// [`TagManager::add_to_tag`]; removing a name that is not in the tag is
    /// recorded anyway, so later re-adds of the static tables stay removed.
    /// Errors when the tag tables are frozen.
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
        // The last operation on a name wins.
        tag.added.retain(|entry| !names_equal(entry, entry_name));
        if !tag.is_removed(entry_name) {
            tag.removed.push(entry_name.to_string());
        }
        Ok(())
    }

    /// Creates a new, initially empty tag. [`TagManager::add_to_tag`] creates
    /// tags implicitly; an explicit create is useful to publish an empty tag.
    /// Errors when the tag tables are frozen.
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

    /// Whether the overlay touches `registry_key` at all.
    #[must_use]
    pub fn has_overlay_for(&self, registry_key: RegistryKey) -> bool {
        self.overlay
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&registry_key)
            .is_some_and(|tags| !tags.is_empty())
    }

    /// Merged view of one tag's entry names: the static table (latest
    /// bundled version) plus overlay additions, minus overlay removals.
    /// Names keep their static-table form (bare paths); overlay additions
    /// keep the form the plugin used. Empty when neither the static tables
    /// nor the overlay know the tag.
    #[must_use]
    pub fn get_tag_values(&self, registry_key: RegistryKey, tag_name: &str) -> Vec<String> {
        let static_names = pumpkin_data::tag::get_tag_values(registry_key, tag_name);
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

    /// The registry keys whose tags must be sent to a client of `version`:
    /// every network-synced key with a non-empty static table, plus every key
    /// the overlay touches. Without an overlay this is exactly the key set
    /// the server sent before tags became plugin-writable.
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

    /// Builds the merged tag maps sent to a client of `version`: for every
    /// registry key the overlay touches, the static table (for that version)
    /// plus overlay additions, minus overlay removals, with every entry
    /// resolved to its final wire id.
    ///
    /// Id resolution keeps the tag packet aligned with the registry data the
    /// client received first: static entries and vanilla additions resolve to
    /// their bundled-registry ids and are remapped for the client version
    /// exactly like the static path; custom additions resolve through
    /// [`RegistryManager::custom_network_id`] (the version's vanilla entry
    /// count plus the entry's registration index) and are written verbatim.
    ///
    /// Returns `None` when the overlay is empty, letting callers fall back to
    /// the unmodified static-table serialization.
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

    /// Merges one static tag with its overlay: static ids (remapped for the
    /// client version) minus removals, plus resolved additions.
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
                // A name already in the static tag was never really added.
                if tag.0.iter().any(|name| names_equal(name, entry)) {
                    continue;
                }
                self.push_resolved(&mut ids, key, entry, version);
            }
        }
        ids
    }

    /// Resolves every entry of an overlay-only (new) tag.
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

    /// Resolves `entry` for `version` and pushes its final wire id onto
    /// `ids`, applying version remapping to bundled-registry ids and passing
    /// custom registry ids through. Unknown entries are skipped with a
    /// warning so one bad plugin entry cannot break the whole tag packet.
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
                "Tag overlay: unknown {} entry '{entry}'; skipping",
                key.identifier_string(),
            );
        }
    }

    /// Resolves an entry name to its network id for `version`. The boolean is
    /// `true` when the id is a bundled-registry id that still needs version
    /// remapping, and `false` when it is a final per-version id (a custom
    /// registry entry, which the registry sync placed after the vanilla
    /// entries and which must pass through verbatim).
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

/// The id of a vanilla entry in the newest bundled registry (`REGISTRY_V_26_3`
/// for synced registries, the static classes for hardcoded ones) — the same
/// id base the static tag tables use, so the caller's version remapping
/// applies to it exactly like to static-table ids.
///
/// `point_of_interest_type` has no bundled name table, so vanilla additions
/// to its tags cannot be resolved (they are skipped with a warning); custom
/// additions still work.
fn vanilla_entry_id(key: RegistryKey, path: &str) -> Option<u16> {
    match key {
        RegistryKey::Block => {
            pumpkin_data::Block::from_registry_key(path).map(|block| block.id.as_u16())
        }
        RegistryKey::Item => pumpkin_data::item::Item::from_registry_key(path).map(|item| item.id),
        RegistryKey::Fluid => {
            pumpkin_data::fluid::Fluid::from_registry_key(path).map(|fluid| fluid.id)
        }
        RegistryKey::EntityType => {
            pumpkin_data::entity::EntityType::from_name(path).map(|entity| entity.id)
        }
        RegistryKey::GameEvent => {
            pumpkin_data::game_event::GameEvent::from_name(path).map(|event| event as u16)
        }
        RegistryKey::Potion => {
            pumpkin_data::potion::Potion::from_name(path).map(|potion| u16::from(potion.id))
        }
        _ => pumpkin_data::registry::REGISTRY_V_26_3
            .iter()
            .find(|registry| registry.registry_id == key.identifier_string())
            .and_then(|registry| registry.entries.iter().position(|entry| entry.name == path))
            .and_then(|index| u16::try_from(index).ok()),
    }
}

/// Version remapping applied to bundled-registry ids, identical to the
/// static-path remapping inside the tag packets: item and entity type ids are
/// remapped across versions, every other registry's ids pass through.
fn remap_tag_entry_id(key: RegistryKey, id: u16, version: JavaMinecraftVersion) -> u16 {
    match key {
        RegistryKey::Item => pumpkin_data::item_id_remap::remap_item_id_for_version(id, version),
        RegistryKey::EntityType => {
            pumpkin_data::entity_id_remap::remap_entity_id_for_version(id, version)
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
        // Static entries come first, additions after them.
        assert_eq!(values.last().unwrap(), "minecraft:stone");
    }

    #[test]
    fn bare_and_namespaced_names_are_the_same_entry() {
        let (_registries, tags) = manager();
        tags.add_to_tag(RegistryKey::Item, "minecraft:anvil", "stone")
            .unwrap();
        // Adding the same entry with the explicit namespace is a no-op.
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
        // add -> remove -> gone
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
        // remove(static) -> add -> back
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

        let anvil = pumpkin_data::item::Item::from_registry_key("anvil")
            .unwrap()
            .id;
        let chipped = pumpkin_data::item::Item::from_registry_key("chipped_anvil")
            .unwrap()
            .id;
        let stone = pumpkin_data::item::Item::from_registry_key("stone")
            .unwrap()
            .id;
        // 26.3 is the bundled version: remapping is the identity.
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

        let stone_native = pumpkin_data::item::Item::from_registry_key("stone")
            .unwrap()
            .id;
        let stone_1_20_2 =
            pumpkin_data::item_id_remap::remap_item_id_for_version(stone_native, version);
        assert!(ids.contains(&stone_1_20_2));
        // Static ids are remapped the same way.
        let anvil_native = pumpkin_data::item::Item::from_registry_key("anvil")
            .unwrap()
            .id;
        let anvil_1_20_2 =
            pumpkin_data::item_id_remap::remap_item_id_for_version(anvil_native, version);
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

        // The id equals the vanilla entry count of the synced registry plus
        // the registration index, matching `inject_custom_entries`.
        let vanilla_count = pumpkin_data::registry::Registry::get_synced(version)
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
        // `RegistryManager` hands out network ids only for synced registries
        // (item/block/... are hardcoded and never synced through
        // `CRegistryData`), so a custom item name has no network id and the
        // snapshot skips it with a warning instead of inventing one.
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
        let stone = pumpkin_data::item::Item::from_registry_key("stone")
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
        let stone = pumpkin_data::item::Item::from_registry_key("stone")
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
        // Without an overlay the helper reproduces the pre-overlay key set.
        assert_eq!(tags.network_tag_keys(version), baseline);

        // A key that is network-valid for this version but has no static
        // table (e.g. potion on 1.21) still gets sent once the overlay
        // touches it. Keys invalid for the version must stay unsent, so
        // restrict the search to valid ones.
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
        // damage_type is a synced registry without a hardcoded name table.
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
        let arrow = pumpkin_data::registry::REGISTRY_V_26_3
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
