use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::tag::{
    HostTagManager, TagManager as WitTagManager,
};
use pumpkin_data::tag::RegistryKey;
use wasmtime::component::Resource;

impl HostTagManager for PluginHostState {
    async fn add_to_tag(
        &mut self,
        _res: Resource<WitTagManager>,
        registry_key: String,
        tag_name: String,
        entry_name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(key) = parse_registry_key(&registry_key) else {
            return Ok(Err(format!("Unknown registry key '{registry_key}'")));
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        // A frozen tag table (plugin loading finished) is reported as `Err`.
        Ok(server.tag_manager.add_to_tag(key, &tag_name, &entry_name))
    }

    async fn remove_from_tag(
        &mut self,
        _res: Resource<WitTagManager>,
        registry_key: String,
        tag_name: String,
        entry_name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(key) = parse_registry_key(&registry_key) else {
            return Ok(Err(format!("Unknown registry key '{registry_key}'")));
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        Ok(server
            .tag_manager
            .remove_from_tag(key, &tag_name, &entry_name))
    }

    async fn create_tag(
        &mut self,
        _res: Resource<WitTagManager>,
        registry_key: String,
        tag_name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(key) = parse_registry_key(&registry_key) else {
            return Ok(Err(format!("Unknown registry key '{registry_key}'")));
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        Ok(server.tag_manager.create_tag(key, &tag_name))
    }

    async fn get_tag_values(
        &mut self,
        _res: Resource<WitTagManager>,
        registry_key: String,
        tag_name: String,
    ) -> wasmtime::Result<Vec<String>> {
        // An unknown registry key yields an empty list, mirroring the "empty
        // when the tag is unknown" contract of the WIT interface.
        let Some(key) = parse_registry_key(&registry_key) else {
            return Ok(Vec::new());
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        Ok(server.tag_manager.get_tag_values(key, &tag_name))
    }

    async fn drop(&mut self, _rep: Resource<WitTagManager>) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// Parses a registry key identifier (e.g. `"item"`, `"block"`, `"damage_type"`;
/// a `"minecraft:"` prefix is accepted) to its [`RegistryKey`].
#[must_use]
pub fn parse_registry_key(key: &str) -> Option<RegistryKey> {
    let key = key.strip_prefix("minecraft:").unwrap_or(key);
    RegistryKey::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.identifier_string() == key)
}
