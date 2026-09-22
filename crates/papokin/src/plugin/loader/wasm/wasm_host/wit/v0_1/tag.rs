use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::tag::{
    HostTagManager, TagManager as WitTagManager,
};
use papokin_data::tag::RegistryKey;
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
            return Ok(Err(format!("未知的注册表键 '{registry_key}'")));
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        // 冻结的标签表（插件加载已完成）会以 `Err` 报告。
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
            return Ok(Err(format!("未知的注册表键 '{registry_key}'")));
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
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
            return Ok(Err(format!("未知的注册表键 '{registry_key}'")));
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.tag_manager.create_tag(key, &tag_name))
    }

    async fn get_tag_values(
        &mut self,
        _res: Resource<WitTagManager>,
        registry_key: String,
        tag_name: String,
    ) -> wasmtime::Result<Vec<String>> {
        // 未知的注册表键返回空列表，与 "empty
        // 当标签未知时"这一 WIT 接口契约。
        let Some(key) = parse_registry_key(&registry_key) else {
            return Ok(Vec::new());
        };
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.tag_manager.get_tag_values(key, &tag_name))
    }

    async fn drop(&mut self, _rep: Resource<WitTagManager>) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// 解析注册表键标识符（例如 `"item"`、`"block"`、`"damage_type"`；
/// （接受带 `"minecraft:"` 前缀的形式）映射到其 [`RegistryKey`]。
#[must_use]
pub fn parse_registry_key(key: &str) -> Option<RegistryKey> {
    let key = key.strip_prefix("minecraft:").unwrap_or(key);
    RegistryKey::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.identifier_string() == key)
}
