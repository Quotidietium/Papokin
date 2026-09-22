use crate::plugin::loader::wasm::wasm_host::{
    logging::log_tracing, state::PluginHostState, wit::v0_1::papokin,
};

impl papokin::plugin::logging::Host for PluginHostState {
    async fn log(
        &mut self,
        level: papokin::plugin::logging::Level,
        message: String,
    ) -> wasmtime::Result<()> {
        match level {
            papokin::plugin::logging::Level::Trace => tracing::trace!("[plugin] {message}"),
            papokin::plugin::logging::Level::Debug => tracing::debug!("[plugin] {message}"),
            papokin::plugin::logging::Level::Info => tracing::info!("[plugin] {message}"),
            papokin::plugin::logging::Level::Warn => tracing::warn!("[plugin] {message}"),
            papokin::plugin::logging::Level::Error => tracing::error!("[plugin] {message}"),
        }
        Ok(())
    }

    async fn log_tracing(&mut self, event: Vec<u8>) -> wasmtime::Result<()> {
        log_tracing(event).await;
        Ok(())
    }
}
