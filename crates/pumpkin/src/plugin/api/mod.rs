pub mod context;
pub mod events;
pub mod gui;
pub mod tab_list;
pub mod title;

use std::{pin::Pin, sync::Arc};

pub use context::*;
pub use events::*;
pub use tab_list::*;
pub use title::*;

/// Struct representing metadata for a plugin.
///
/// This struct contains essential information about a plugin, including its name,
/// version, authors, and a description. It is generic over a lifetime `'s` to allow
/// for string slices that are valid for the lifetime of the plugin metadata.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// The name of the plugin.
    pub name: String,
    /// The version of the plugin.
    pub version: String,
    /// The authors of the plugin.
    pub authors: Vec<String>,
    /// A description of the plugin.
    pub description: String,
    /// Hard dependencies: the plugin fails to load when any of these is missing.
    pub dependencies: Vec<String>,
    /// The permissions requested by the plugin.
    pub permissions: Vec<String>,
    /// Soft ordering edges: load this plugin after the named plugins when they
    /// are present. Missing names are ignored.
    pub load_after: Vec<String>,
    /// Soft ordering edges: load this plugin before the named plugins when they
    /// are present. Missing names are ignored.
    pub load_before: Vec<String>,
    /// Capability aliases this plugin satisfies for other plugins' dependency
    /// edges.
    pub provides: Vec<String>,
    /// The startup phase this plugin loads in.
    pub load_order: LoadOrder,
}

/// When a plugin should be loaded relative to server startup.
///
/// Defaults to [`LoadOrder::PostWorld`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadOrder {
    /// Load before worlds are created (bootstrap phase).
    Startup,
    /// Load after worlds are ready (default).
    #[default]
    PostWorld,
}

/// This type represents a future for the plugin.
pub type PluginFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait representing a plugin with asynchronous lifecycle methods.
///
/// This trait defines the required methods for a plugin, including hooks for when
/// the plugin is loaded and unloaded.
pub trait Plugin: Send + Sync + 'static {
    /// Asynchronous method called when the plugin is loaded.
    ///
    /// This method initializes the plugin within the server context.
    #[expect(unused)]
    fn on_load(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// Asynchronous method called to enable the plugin after a successful load.
    ///
    /// A failing enable does not unload the plugin (unlike [`Plugin::on_load`]):
    /// the plugin stays loaded but inactive — its event handlers and commands
    /// are unregistered — mirroring Paper's `onEnable` failure grading.
    #[expect(unused)]
    fn on_enable(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// Asynchronous method called to disable an active plugin.
    ///
    /// Runs before [`Plugin::on_unload`] during shutdown/unload, and when the
    /// plugin is explicitly disabled.
    #[expect(unused)]
    fn on_disable(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// Asynchronous method called when the plugin is unloaded.
    ///
    /// This method cleans up resources when the plugin is removed from the server context.
    #[expect(unused)]
    fn on_unload(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// Asynchronous method called when the plugin receives an IPC message.
    ///
    /// This processes the message, and optionally returns a response
    #[expect(unused)]
    fn on_ipc_message(
        &self,
        sender: &str,
        message: &[u8],
    ) -> PluginFuture<'_, Result<Vec<u8>, String>> {
        Box::pin(async move { Err("This plugin cannot receive messages.".to_string()) })
    }

    /// Asynchronous method called when a player sends a plugin message on a
    /// channel this plugin registered (Bukkit `PluginMessageListener`).
    #[allow(unused_variables)]
    fn on_plugin_message(
        &self,
        _player_uuid: uuid::Uuid,
        _channel: &str,
        _data: &[u8],
    ) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }
}
