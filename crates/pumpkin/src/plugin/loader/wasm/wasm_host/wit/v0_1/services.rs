use crate::plugin::loader::wasm::wasm_host::{state::PluginHostState, wit::v0_1::pumpkin};

#[allow(clippy::unused_async_trait_impl)]
impl pumpkin::plugin::services::Host for PluginHostState {
    async fn register_service(
        &mut self,
        service: String,
        priority: i32,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Err("plugin name is not set".to_string()));
        };
        let Some(server) = self.server.as_ref() else {
            return Ok(Err("server is not set".to_string()));
        };
        server
            .plugin_manager
            .register_service_provider(&service, &name, priority);
        Ok(Ok(()))
    }

    async fn unregister_service(&mut self, service: String) -> wasmtime::Result<()> {
        if let (Some(name), Some(server)) = (self.name.clone(), self.server.as_ref()) {
            server
                .plugin_manager
                .unregister_service_provider(&service, &name);
        }
        Ok(())
    }

    async fn get_service_provider(
        &mut self,
        service: String,
    ) -> wasmtime::Result<Option<pumpkin::plugin::services::ServiceProvider>> {
        Ok(self
            .get_service_providers(service)
            .await?
            .into_iter()
            .next())
    }

    async fn get_service_providers(
        &mut self,
        service: String,
    ) -> wasmtime::Result<Vec<pumpkin::plugin::services::ServiceProvider>> {
        let Some(server) = self.server.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(server
            .plugin_manager
            .get_service_providers(&service)
            .into_iter()
            .map(|r| pumpkin::plugin::services::ServiceProvider {
                service: r.service,
                plugin: r.plugin,
                priority: r.priority,
            })
            .collect())
    }
}
