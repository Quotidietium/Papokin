use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::registry::{
    HostRegistryManager, RegistryManager as WitRegistryManager,
};
use wasmtime::component::Resource;

impl HostRegistryManager for PluginHostState {
    async fn register_entry(
        &mut self,
        _res: Resource<WitRegistryManager>,
        domain: String,
        name: String,
        nbt: Vec<u8>,
    ) -> wasmtime::Result<Result<u16, String>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        // 冻结的注册表（插件加载已完成）或重复名称
        // 以 `Err` 报告；注册绝不 panic。
        Ok(server.registry_manager.register(&domain, name, nbt))
    }

    async fn custom_network_id(
        &mut self,
        _res: Resource<WitRegistryManager>,
        domain: String,
        name: String,
    ) -> wasmtime::Result<Option<u16>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.registry_manager.custom_network_id(
            &domain,
            &name,
            papokin_data::packet::CURRENT_MC_VERSION,
        ))
    }

    async fn is_frozen(&mut self, _res: Resource<WitRegistryManager>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.registry_manager.is_frozen())
    }

    async fn has_entry(
        &mut self,
        _res: Resource<WitRegistryManager>,
        domain: String,
        name: String,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.registry_manager.index_of(&domain, &name).is_some())
    }

    async fn get_entries(
        &mut self,
        _res: Resource<WitRegistryManager>,
        domain: String,
    ) -> wasmtime::Result<Vec<String>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server
            .registry_manager
            .entries_for(&domain)
            .into_iter()
            .map(|entry| entry.name)
            .collect())
    }

    async fn drop(&mut self, _rep: Resource<WitRegistryManager>) -> wasmtime::Result<()> {
        Ok(())
    }
}
