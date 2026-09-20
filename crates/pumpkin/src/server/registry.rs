//! Plugin-writable registry: custom entries for synced registries.
//!
//! Plugins register custom entries (e.g. new damage types) with the
//! [`RegistryManager`] while the server is starting up. When a client
//! connects, the entries are appended to the vanilla synced registries sent
//! in the registry data packet, so a custom entry's network id for a given
//! protocol version is that version's vanilla entry count plus the entry's
//! registration index. Registration closes when [`RegistryManager::freeze`]
//! is called once plugin loading has finished.
//!
//! Only protocol versions that sync registries through the `CRegistryData`
//! packet (1.20.2+) receive custom entries this way; older clients use the
//! login registry codec (`build_v1_20_registry_codec`), which is not extended
//! here.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{PoisonError, RwLock};

use pumpkin_data::registry::{Registry, RegistryEntryData};
use pumpkin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashMap;

/// One plugin-registered custom registry entry: its namespaced id and the
/// serialized NBT payload sent to clients in the registry data packet.
#[derive(Clone, Debug)]
pub struct CustomRegistryEntry {
    pub name: String,
    // e.g. "myplugin:frost"
    pub nbt: Vec<u8>,
    // serialized NBT compound (network format)
}

/// Holds plugin-registered custom entries for synced registries and hands
/// out network ids.
///
/// Entries keep registration order; a custom entry's
/// network id for a given protocol version is that version's vanilla entry
/// count plus the entry's index. Registration is allowed until `freeze`
/// flips when plugin loading finishes.
pub struct RegistryManager {
    /// Custom entries per domain (e.g. "`damage_type`"), in registration order.
    custom: RwLock<FxHashMap<String, Vec<CustomRegistryEntry>>>,
    /// Set once plugin loading has finished; further registrations fail.
    frozen: AtomicBool,
    /// Vanilla entry counts per domain, cached per protocol version (keyed by
    /// `JavaMinecraftVersion::protocol_version`) so network id lookup does not
    /// re-clone the synced registry tables on every call.
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

    /// Registers a custom entry under `domain` (e.g. "`damage_type`", without
    /// the "minecraft:" prefix). Returns the entry's index within the domain.
    /// Errors when frozen or when the name duplicates an existing custom entry.
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

    /// All custom entries of a domain, in registration order.
    #[must_use]
    pub fn entries_for(&self, domain: &str) -> Vec<CustomRegistryEntry> {
        self.custom
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(domain)
            .cloned()
            .unwrap_or_default()
    }

    /// Index of a custom entry by name (its offset after the vanilla entries).
    #[must_use]
    pub fn index_of(&self, domain: &str, name: &str) -> Option<u16> {
        let custom = self.custom.read().unwrap_or_else(PoisonError::into_inner);
        custom
            .get(domain)?
            .iter()
            .position(|entry| entry.name == name)
            .and_then(|index| u16::try_from(index).ok())
    }

    /// Name of the custom entry at `index`, if any.
    #[must_use]
    pub fn name_at(&self, domain: &str, index: u16) -> Option<String> {
        let custom = self.custom.read().unwrap_or_else(PoisonError::into_inner);
        custom
            .get(domain)?
            .get(usize::from(index))
            .map(|entry| entry.name.clone())
    }

    /// Network id of a custom entry for a client protocol version: the version's
    /// vanilla entry count plus the custom index. Returns None for unknown
    /// domains or names. The vanilla count is looked up from the synced static
    /// tables via `Registry::get_synced(version)` and cached per version.
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

    /// Vanilla entry count of `domain` for `version`, from the synced static
    /// tables. Counts for every domain of a version are cached on first use.
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
        // A racing thread may have cached the same version already; the static
        // tables are identical either way, so overwriting is harmless.
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

/// The registry domain of a synced registry id: the id without its
/// "minecraft:" prefix (e.g. "`damage_type`" for "`minecraft:damage_type`").
fn domain_of(registry_id: &str) -> &str {
    registry_id
        .strip_prefix("minecraft:")
        .unwrap_or(registry_id)
}

/// Appends every plugin-registered custom entry to its synced registry.
///
/// Entries are appended in registration order, so they are sent to the
/// client right after the vanilla ones. Registries without custom entries
/// are left untouched.
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
        // The same name in a different domain is a different entry.
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
            .expect("the 1.21 table has a damage_type registry");
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
        // Unknown names and domains yield no id.
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

        // A second version exercises the per-version cache; the 1.20 table
        // differs from 1.21, so this also proves the versions do not alias.
        let vanilla_1_20 = Registry::get_synced(JavaMinecraftVersion::V_1_20)
            .iter()
            .find(|reg| reg.registry_id == "minecraft:damage_type")
            .map(|reg| reg.registry_entries.len())
            .expect("the 1.20 table has a damage_type registry");
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

        // Domains without custom entries are untouched.
        let biome = registries
            .iter()
            .find(|reg| reg.registry_id == "minecraft:worldgen/biome")
            .unwrap();
        assert_eq!(biome.registry_entries.len(), biome_count);
    }
}
