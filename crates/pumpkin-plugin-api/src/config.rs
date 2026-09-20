//! Plugin configuration files (Bukkit `getConfig` equivalent).
//!
//! The host stores your plugin's config at `<data folder>/config.toml`. Call
//! [`Context::load_config`] to read it merged over the defaults returned by
//! [`Plugin::config_defaults`](crate::Plugin::config_defaults) — new default
//! keys are added automatically and user values survive plugin upgrades.
//!
//! # Example
//!
//! ```rust,ignore
//! impl Plugin for MyPlugin {
//!     fn config_defaults(&self) -> String {
//!         "welcome-message = \"Hello!\"\nmax-teleports = 5\n".into()
//!     }
//! }
//!
//! // Somewhere in the plugin:
//! let config = context.load_config()?;
//! ```
//!
//! Parse the returned TOML with the `toml` crate (or any TOML parser).

use crate::Context;

impl Context {
    /// Loads the plugin's config, merged over the TOML `defaults` string.
    ///
    /// The merged document is persisted back to disk.
    ///
    /// # Errors
    /// Returns an error when either document is not valid TOML.
    pub fn load_config_with_defaults(&self, defaults: &str) -> crate::Result<String> {
        crate::wit::pumpkin::plugin::config::load_config(defaults)
    }

    /// Loads the plugin's config merged over [`Plugin::config_defaults`](crate::Plugin::config_defaults).
    ///
    /// The merged document is persisted back to disk.
    ///
    /// # Errors
    /// Returns an error when either document is not valid TOML.
    pub fn load_config(&self) -> crate::Result<String> {
        let defaults = crate::config_defaults();
        crate::wit::pumpkin::plugin::config::load_config(&defaults)
    }

    /// Overwrites the plugin's config file with the given TOML content.
    ///
    /// # Errors
    /// Returns an error when the content is not valid TOML or writing fails.
    pub fn save_config(&self, content: &str) -> crate::Result<()> {
        crate::wit::pumpkin::plugin::config::save_config(content)
    }
}
