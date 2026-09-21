//! Runtime registration of plugin-provided custom damage types.
//!
//! Plugins register new damage types with the [`DamageTypeManager`] while the
//! server is starting up (before [`crate::server::registry::RegistryManager`]
//! is frozen at the end of plugin loading). A registration does two things:
//!
//! - the entry is appended to the `damage_type` domain of the
//!   [`crate::server::registry::RegistryManager`], so it is synced to clients
//!   after the vanilla entries (its network id for a client version is that
//!   version's vanilla entry count plus the registration index),
//! - a [`CustomDamageType`] descriptor is kept so the damage execution path
//!   can resolve names back to their fields (death message id, exhaustion,
//!   ...) through [`DamageTypeManager::resolve`].
//!
//! Vanilla damage types are untouched: they keep resolving to the static
//! [`DamageType`] table.

use std::sync::{PoisonError, RwLock};

use pumpkin_data::damage::{DamageEffects, DamageScaling, DamageType, DeathMessageType};
use pumpkin_data::damage_ext::{
    CustomDamageType, ResolvedDamageType, custom_damage_type_nbt, native_damage_type_count,
};
use rustc_hash::FxHashMap;

use crate::server::Server;

/// The synced-registry domain custom damage types are registered under.
pub const DAMAGE_TYPE_DOMAIN: &str = "damage_type";

/// Registration input for a custom damage type. The fields mirror the vanilla
/// `damage_type` registry entry structure.
#[derive(Clone, Debug)]
pub struct CustomDamageTypeDefinition {
    /// Namespaced id of the entry, e.g. "`myplugin:frost`".
    pub name: String,
    /// Death message id; death messages translate as `death.attack.<message_id>`.
    pub message_id: String,
    /// When the damage amount scales with difficulty.
    pub scaling: DamageScaling,
    /// Exhaustion applied to players taking this damage.
    pub exhaustion: f32,
    /// Optional hurt sound/visual effect override.
    pub effects: Option<DamageEffects>,
    /// How the death message is composed.
    pub death_message_type: DeathMessageType,
}

/// Consistent snapshot of the registered custom damage types: lookup by name
/// plus the registration order, which fixes the network ids (native vanilla
/// count + registration index) assigned by the registry sync.
struct CustomDamageTypeState {
    by_name: FxHashMap<String, CustomDamageType>,
    order: Vec<String>,
}

/// Holds plugin-registered custom damage types and resolves damage type
/// identifiers (vanilla names or custom namespaced ids) to the representation
/// the damage execution path consumes.
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

    /// Registers a custom damage type end to end: syncs it to clients through
    /// `server.registry_manager` (appended after the vanilla entries of the
    /// `damage_type` registry) and keeps its descriptor for name resolution.
    /// Returns the entry's network id in the native dataset id space (native
    /// vanilla count + registration index).
    ///
    /// # Errors
    /// Duplicate names, vanilla name collisions, or a frozen registry
    /// (plugin loading finished) are reported as `Err`.
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
        // A name in the minecraft namespace (or without any namespace) must
        // not shadow a vanilla damage type.
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

    /// Resolves a damage type identifier to the representation the damage
    /// execution path consumes. Vanilla names (with or without the
    /// "minecraft:" prefix, e.g. "arrow" or "minecraft:arrow") resolve to the
    /// static [`DamageType`] table; anything else is looked up among the
    /// registered custom damage types by its full namespaced id.
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

    /// The registered custom damage type with this namespaced id, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<CustomDamageType> {
        self.custom
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .by_name
            .get(name)
            .cloned()
    }

    /// All registered custom damage types, in registration (network id) order.
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
