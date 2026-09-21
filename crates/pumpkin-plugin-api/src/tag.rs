//! Plugin-writable tag overlay utilities.
//!
//! Tags are compiled into the server as static, per-version tables. Plugins use the
//! [`TagManager`] while the server is starting up to append entries to existing tags,
//! remove entries from them, or create entirely new tags; the merged result is sent to
//! connecting clients in the update-tags packets.
//!
//! Modifications are only possible while the server is starting up (plugin loading);
//! once the tag tables are frozen, all mutating calls fail with a [`TagError`].
//!
//! # Examples
//!
//! ```rust,ignore
//! use pumpkin_plugin_api::Context;
//!
//! fn register_tags(context: &Context) {
//!     let manager = context.get_tag_manager();
//!     manager
//!         .add("damage_type", "my_plugin:custom_damage", "my_plugin:frost")
//!         .expect("failed to add to tag");
//!     assert!(manager
//!         .get_values("damage_type", "my_plugin:custom_damage")
//!         .iter()
//!         .any(|name| name == "my_plugin:frost"));
//! }
//! ```

use crate::Context;
pub use crate::wit::pumpkin::plugin::tag::TagManager;
use std::fmt;

/// Errors that can occur when modifying a tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagError {
    /// Adding an entry to a tag failed (e.g. unknown registry key or frozen tag tables).
    AddFailed(String),
    /// Removing an entry from a tag failed (e.g. unknown registry key or frozen tag tables).
    RemoveFailed(String),
    /// Creating a tag failed (e.g. unknown registry key or frozen tag tables).
    CreateFailed(String),
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddFailed(msg) => write!(f, "failed to add entry to tag: {msg}"),
            Self::RemoveFailed(msg) => write!(f, "failed to remove entry from tag: {msg}"),
            Self::CreateFailed(msg) => write!(f, "failed to create tag: {msg}"),
        }
    }
}

impl std::error::Error for TagError {}

impl TagManager {
    /// Appends `entry_name` to the tag `tag_name` of `registry_key`, creating the tag
    /// when it does not exist yet.
    ///
    /// * `registry_key`: the registry the tag belongs to, e.g. `"item"`, `"block"` or
    ///   `"damage_type"` (a `"minecraft:"` prefix is accepted).
    /// * `tag_name`: namespaced tag name, e.g. `"minecraft:anvil"` or `"my_plugin:things"`.
    /// * `entry_name`: resource name of the entry: a vanilla name (`"minecraft:stone"`
    ///   or bare `"stone"`) or a plugin-registered custom namespaced id (`"my_plugin:frost"`).
    ///
    /// # Errors
    /// Returns [`TagError::AddFailed`] when the registry key is unknown or the tag
    /// tables are frozen.
    pub fn add(
        &self,
        registry_key: &str,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), TagError> {
        self.add_to_tag(registry_key, tag_name, entry_name)
            .map_err(TagError::AddFailed)
    }

    /// Removes `entry_name` from the tag `tag_name` of `registry_key`. The entry may
    /// come from the static tables or from a previous [`TagManager::add`].
    ///
    /// # Errors
    /// Returns [`TagError::RemoveFailed`] when the registry key is unknown or the tag
    /// tables are frozen.
    pub fn remove(
        &self,
        registry_key: &str,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), TagError> {
        self.remove_from_tag(registry_key, tag_name, entry_name)
            .map_err(TagError::RemoveFailed)
    }

    /// Creates a new, initially empty tag. [`TagManager::add`] creates tags
    /// implicitly; an explicit create is useful to publish an empty tag.
    ///
    /// # Errors
    /// Returns [`TagError::CreateFailed`] when the registry key is unknown or the tag
    /// tables are frozen.
    pub fn create(&self, registry_key: &str, tag_name: &str) -> Result<(), TagError> {
        self.create_tag(registry_key, tag_name)
            .map_err(TagError::CreateFailed)
    }

    /// Merged view of one tag's entry names: the static table plus plugin additions,
    /// minus plugin removals. Empty when the tag is unknown.
    #[must_use]
    pub fn get_values(&self, registry_key: &str, tag_name: &str) -> Vec<String> {
        self.get_tag_values(registry_key, tag_name)
    }
}

impl Context {
    /// Returns the global tag manager for modifying and querying the tag tables.
    #[must_use]
    pub fn get_tag_manager(&self) -> TagManager {
        self.get_server().get_tag_manager()
    }

    /// Appends `entry_name` to the tag `tag_name` of `registry_key`, creating the tag
    /// when it does not exist yet.
    ///
    /// # Errors
    /// Returns [`TagError::AddFailed`] when the registry key is unknown or the tag
    /// tables are frozen.
    pub fn add_to_tag(
        &self,
        registry_key: &str,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), TagError> {
        self.get_tag_manager()
            .add(registry_key, tag_name, entry_name)
    }
}
