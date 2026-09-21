//! Plugin-writable custom registry entry utilities.
//!
//! Plugins register custom entries (e.g. new damage types) with the [`RegistryManager`]
//! while the server is starting up. When a client connects, the entries are appended to
//! the vanilla synced registries sent in the registry data packet, so a custom entry's
//! network id is the vanilla entry count plus the entry's registration index.
//!
//! Registration is only possible while the server is starting up (plugin loading);
//! once the registries are frozen, registration fails with
//! [`RegistryError::RegistrationFailed`].
//!
//! Most plugins should prefer the dedicated managers built on top of this one
//! ([`DamageTypeManager`](crate::damage_type::DamageTypeManager), the enchantment
//! manager) which serialize the entry payload for you; this manager is the generic
//! fallback for registries without a dedicated API.
//!
//! # Examples
//!
//! ```rust,ignore
//! use pumpkin_plugin_api::Context;
//!
//! fn register_entries(context: &Context) {
//!     let manager = context.get_registry_manager();
//!     let index = manager
//!         .register("damage_type", "my_plugin:frost", serialized_nbt)
//!         .expect("failed to register registry entry");
//!     assert!(manager.has("damage_type", "my_plugin:frost"));
//!     assert_eq!(manager.network_id("damage_type", "my_plugin:frost").is_some(), true);
//! }
//! ```

pub use crate::wit::pumpkin::plugin::registry::RegistryManager;
use crate::{Context, Server};
use std::fmt;

/// Errors that can occur when registering a custom registry entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Registration with the server registry manager failed (e.g. duplicate name or
    /// frozen registry).
    RegistrationFailed(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RegistrationFailed(msg) => {
                write!(f, "failed to register registry entry: {msg}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

impl RegistryManager {
    /// Registers a custom entry under `domain` (e.g. `"damage_type"`, without the
    /// `"minecraft:"` prefix) with its serialized NBT payload (network format).
    ///
    /// Returns the entry's index within the domain; the entry's network id for a
    /// client is that client's vanilla entry count plus this index.
    ///
    /// # Errors
    /// Returns [`RegistryError::RegistrationFailed`] when the name duplicates an
    /// existing custom entry or the registry is frozen.
    pub fn register(&self, domain: &str, name: &str, nbt: Vec<u8>) -> Result<u16, RegistryError> {
        self.register_entry(domain, name, &nbt)
            .map_err(RegistryError::RegistrationFailed)
    }

    /// Network id of a custom entry for the server's native protocol version: the
    /// vanilla entry count of the domain plus the entry's registration index.
    ///
    /// Returns `None` for unknown domains or names, and for registries that are not
    /// synced to clients.
    #[must_use]
    pub fn network_id(&self, domain: &str, name: &str) -> Option<u16> {
        self.custom_network_id(domain, name)
    }

    /// Whether a custom entry with this name is registered under `domain`.
    #[must_use]
    pub fn has(&self, domain: &str, name: &str) -> bool {
        self.has_entry(domain, name)
    }

    /// Names of all custom entries of a domain, in registration order.
    #[must_use]
    pub fn names(&self, domain: &str) -> Vec<String> {
        self.get_entries(domain)
    }
}

impl Context {
    /// Returns the global registry manager for registering and querying custom
    /// registry entries.
    #[must_use]
    pub fn get_registry_manager(&self) -> RegistryManager {
        self.get_server().get_registry_manager()
    }

    /// Registers a custom registry entry with the server.
    ///
    /// # Errors
    /// Returns [`RegistryError::RegistrationFailed`] when the name duplicates an
    /// existing custom entry or the registry is frozen.
    pub fn register_registry_entry(
        &self,
        domain: &str,
        name: &str,
        nbt: Vec<u8>,
    ) -> Result<u16, RegistryError> {
        self.get_registry_manager().register(domain, name, nbt)
    }
}

impl Server {
    /// Registers a custom registry entry with the server.
    ///
    /// # Errors
    /// Returns [`RegistryError::RegistrationFailed`] when the name duplicates an
    /// existing custom entry or the registry is frozen.
    pub fn register_registry_entry(
        &self,
        domain: &str,
        name: &str,
        nbt: Vec<u8>,
    ) -> Result<u16, RegistryError> {
        self.get_registry_manager().register(domain, name, nbt)
    }
}
