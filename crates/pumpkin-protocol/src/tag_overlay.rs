//! Merged tag maps handed to the tag-sync packets (`CUpdateTags` /
//! `CUpdateTagsPlay`) by the server.
//!
//! Tags are normally serialized straight out of the static, compile-time
//! tables in `pumpkin-data`. Plugins can additionally add entries to existing
//! tags, remove entries, or create entirely new tags through the server's tag
//! manager. The server merges its overlay with the static tables, resolves
//! every entry to the network id the connecting client expects (including
//! cross-version remapping and custom registry entries), and passes the
//! result to the packets as a [`MergedTags`]. When a registry key has no
//! overlay the packets fall back to the static-table path untouched.

use std::collections::HashMap;

use pumpkin_data::tag::RegistryKey;

/// A fully merged tag map for one client protocol version.
///
/// This is the static table plus the server's plugin overlay, with every
/// entry already resolved to its final wire id (version remapping already
/// applied where needed).
#[derive(Clone, Debug, Default)]
pub struct MergedTags {
    /// Merged tags per registry key: tag name -> entry ids in wire order.
    /// Only registry keys whose static table was modified by the overlay
    /// (or that only exist through the overlay) are present.
    pub maps: HashMap<RegistryKey, Vec<(String, Vec<u16>)>>,
}

impl MergedTags {
    /// The merged tags of one registry key, if the overlay touched it.
    #[must_use]
    pub fn get(&self, key: RegistryKey) -> Option<&[(String, Vec<u16>)]> {
        self.maps.get(&key).map(Vec::as_slice)
    }

    /// Whether the overlay produced any merged tag for `key`.
    #[must_use]
    pub fn contains_key(&self, key: RegistryKey) -> bool {
        self.maps.contains_key(&key)
    }

    /// Whether no registry key was touched by the overlay at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}
