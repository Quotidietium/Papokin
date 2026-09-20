//! Cross-plugin service registry (Bukkit `ServicesManager` equivalent).
//!
//! A service is a named capability that one plugin provides and others
//! consume. Because plugins run in isolated sandboxes, consumers look the
//! provider up here and invoke it through [`crate::ipc`] (see
//! [`crate::Context::send_ipc_message`]).
//!
//! # Example
//!
//! ```rust,ignore
//! // Provider (in on_enable):
//! context.register_service("my-plugin:economy", 0)?;
//!
//! // Consumer:
//! if let Some(provider) = context.get_service_provider("my-plugin:economy") {
//!     let reply = context.send_ipc_message(&provider.plugin, b"balance:Steve".to_vec())?;
//! }
//! ```

pub use crate::wit::pumpkin::plugin::services::ServiceProvider;

use crate::Context;

impl Context {
    /// Registers this plugin as a provider of `service`.
    ///
    /// Re-registering the same service updates its priority. The registration
    /// is removed automatically when the plugin is unloaded or disabled.
    ///
    /// # Errors
    /// Returns an error string if the host rejected the registration.
    pub fn register_service(&self, service: &str, priority: i32) -> crate::Result<()> {
        crate::wit::pumpkin::plugin::services::register_service(service, priority)
    }

    /// Removes this plugin's registration for `service`.
    pub fn unregister_service(&self, service: &str) {
        crate::wit::pumpkin::plugin::services::unregister_service(service);
    }

    /// Returns the highest-priority active provider of `service`, if any.
    #[must_use]
    pub fn get_service_provider(&self, service: &str) -> Option<ServiceProvider> {
        crate::wit::pumpkin::plugin::services::get_service_provider(service)
    }

    /// Returns all active providers of `service`, sorted by descending
    /// priority (then registration order).
    #[must_use]
    pub fn get_service_providers(&self, service: &str) -> Vec<ServiceProvider> {
        crate::wit::pumpkin::plugin::services::get_service_providers(service)
    }
}
