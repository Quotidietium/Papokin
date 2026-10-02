//! End-to-end managed-target plugin.
//!
//! A minimal second plugin whose whole lifecycle (disable, enable, unload,
//! load) is driven by the e2e plugin through the plugin-manager API, proving
//! that a third-party plugin can replace the built-in plugman command. Every
//! lifecycle hook logs an `E2E target` marker that the harness asserts in the
//! server log.

use papokin_plugin_api::{Context, LoadOrder, Plugin, PluginMetadata, Result, register_plugin};

struct E2eTargetPlugin;

impl Plugin for E2eTargetPlugin {
    fn new() -> Self {
        Self
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "e2e-target".into(),
            version: "0.1.0".into(),
            authors: vec!["papokin".into()],
            description: "E2E managed-target plugin driven via the plugin-manager API".into(),
            dependencies: vec![],
            permissions: vec![],
            load_after: vec![],
            load_before: vec![],
            provides: vec![],
            load_order: LoadOrder::PostWorld,
        }
    }

    fn on_load(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E target on_load ok");
        Ok(())
    }

    fn on_enable(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E target on_enable ok");
        Ok(())
    }

    fn on_disable(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E target on_disable ok");
        Ok(())
    }

    fn on_unload(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E target on_unload ok");
        Ok(())
    }
}

register_plugin!(E2eTargetPlugin);
