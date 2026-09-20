//! Server-level query utilities: build/version information and offline player lookup.
//!
//! The [`Server`](crate::Server) resource exposes these directly as
//! `get_build_info`, `get_offline_player_by_uuid` and
//! `get_offline_player_by_name`; this module re-exports the record types and
//! adds the same lookups to [`Context`] for convenience.

pub use crate::wit::pumpkin::plugin::server::ServerBuildInfo as BuildInfo;
pub use crate::wit::pumpkin::plugin::server::*;

use crate::Context;

impl Context {
    /// Returns build and version information about the running server.
    #[must_use]
    pub fn get_build_info(&self) -> BuildInfo {
        self.get_server().get_build_info()
    }

    /// Looks up a player by UUID string, online or offline.
    ///
    /// Returns `None` when the player is completely unknown to the server
    /// (not online, no player data file, no user cache entry).
    #[must_use]
    pub fn get_offline_player_by_uuid(&self, uuid: &str) -> Option<OfflinePlayerInfo> {
        self.get_server().get_offline_player_by_uuid(uuid)
    }

    /// Looks up a player by name via the user cache, online or offline.
    ///
    /// Returns `None` when the name is unknown to the server.
    #[must_use]
    pub fn get_offline_player_by_name(&self, name: &str) -> Option<OfflinePlayerInfo> {
        self.get_server().get_offline_player_by_name(name)
    }
}
