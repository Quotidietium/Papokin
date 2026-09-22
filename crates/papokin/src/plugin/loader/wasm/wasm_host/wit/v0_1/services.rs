use crate::plugin::loader::wasm::wasm_host::{state::PluginHostState, wit::v0_1::papokin};

#[allow(clippy::unused_async_trait_impl)]
impl papokin::plugin::services::Host for PluginHostState {
    async fn register_service(
        &mut self,
        service: String,
        priority: i32,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Err("未设置插件名称".to_string()));
        };
        let Some(server) = self.server.as_ref() else {
            return Ok(Err("服务器未设置".to_string()));
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
    ) -> wasmtime::Result<Option<papokin::plugin::services::ServiceProvider>> {
        Ok(self
            .get_service_providers(service)
            .await?
            .into_iter()
            .next())
    }

    async fn get_service_providers(
        &mut self,
        service: String,
    ) -> wasmtime::Result<Vec<papokin::plugin::services::ServiceProvider>> {
        let Some(server) = self.server.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(server
            .plugin_manager
            .get_service_providers(&service)
            .into_iter()
            .map(|r| papokin::plugin::services::ServiceProvider {
                service: r.service,
                plugin: r.plugin,
                priority: r.priority,
            })
            .collect())
    }
}
