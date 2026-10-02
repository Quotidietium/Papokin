use std::collections::HashSet;

use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::plugin_manager::{
    Host as PluginManagerHost, HostPluginManager, HostPluginManagerWithStore,
    PluginInfo as WitPluginInfo, PluginManager as WitPluginManager, PluginState as WitPluginState,
};
use crate::plugin::{PluginMetadata, PluginState, resolve_plugin_file};
use wasmtime::component::{Access, HasSelf, Resource};

impl PluginManagerHost for PluginHostState {}

impl HostPluginManager for PluginHostState {
    async fn drop(&mut self, rep: Resource<WitPluginManager>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<crate::plugin::loader::wasm::wasm_host::state::PluginManagerResource>(
            Resource::new_own(rep.rep()),
        );
        Ok(())
    }
}

impl HostPluginManagerWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn list_plugins(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitPluginManager>,
    ) -> wasmtime::Result<Vec<WitPluginInfo>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        // 查询会经过异步锁（状态表/失败表），必须经 pump_reentry 驱动：
        // 纯 async 宿主方法一旦真正挂起，本存储的驱动器无法在推进
        // 该未来的同时处理路由回调，便与持锁方互相等待。
        plugin
            .store
            .pump_reentry(&mut host, async move {
                let manager = &server.plugin_manager;
                let active: HashSet<String> = manager
                    .active_plugins()
                    .iter()
                    .map(|metadata| metadata.name.clone())
                    .collect();
                let mut infos = Vec::new();
                for metadata in manager.loaded_plugins() {
                    infos.push(build_info(manager, metadata, &active).await);
                }

                // 加载失败的插件已不在实例表中，仅保留失败记录；
                // 以元数据为空的条目并入列表，供管理器展示失败原因。
                for (name, reason) in manager.get_failed_plugins().await {
                    if !infos.iter().any(|info| info.name == name) {
                        infos.push(WitPluginInfo {
                            name,
                            version: String::new(),
                            description: String::new(),
                            authors: Vec::new(),
                            is_active: false,
                            state: WitPluginState::Failed,
                            state_detail: Some(reason),
                        });
                    }
                }
                infos
            })
            .await
    }

    async fn get_plugin(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitPluginManager>,
        name: String,
    ) -> wasmtime::Result<Option<WitPluginInfo>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        plugin
            .store
            .pump_reentry(&mut host, async move {
                let manager = &server.plugin_manager;
                let active: HashSet<String> = manager
                    .active_plugins()
                    .iter()
                    .map(|metadata| metadata.name.clone())
                    .collect();
                if let Some(metadata) = manager
                    .loaded_plugins()
                    .into_iter()
                    .find(|metadata| metadata.name == name)
                {
                    return Some(build_info(manager, metadata, &active).await);
                }

                // 不在实例表中：仅当状态表仍留有记录（加载失败）时
                // 返回元数据为空的条目，否则插件从未尝试加载。
                manager.get_plugin_state(&name).await.map(|state| {
                    let (wit_state, detail) = match state {
                        PluginState::Loading => (WitPluginState::Loading, None),
                        PluginState::Disabled(reason) => (WitPluginState::Disabled, Some(reason)),
                        PluginState::Failed(reason) => (WitPluginState::Failed, Some(reason)),
                        PluginState::Loaded => (WitPluginState::Loaded, None),
                    };
                    WitPluginInfo {
                        is_active: active.contains(&name),
                        name,
                        version: String::new(),
                        description: String::new(),
                        authors: Vec::new(),
                        state: wit_state,
                        state_detail: detail,
                    }
                })
            })
            .await
    }

    async fn load_plugin(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitPluginManager>,
        file_name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        let path = match resolve_plugin_file(&file_name) {
            Ok(path) => path,
            Err(message) => return Ok(Err(message)),
        };
        if !path.is_file() {
            return Ok(Err(format!("插件目录中不存在文件 {file_name}")));
        }

        // 动态加载要走完整的初始化管线（编译/实例化/on-load/on-enable），
        // 期间大量跨存储回调，必须经 pump_reentry 驱动。
        plugin
            .store
            .pump_reentry(&mut host, async move {
                let manager = server.plugin_manager.clone();
                manager
                    .try_load_plugin(&server, &path)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await
    }

    async fn unload_plugin(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitPluginManager>,
        name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let (server, plugin, self_name) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin, state.name.clone())
        };

        // 卸载自身会把 on_disable/on_unload 钩子回压给正阻塞在本次
        // 宿主调用中的本插件存储驱动器，必然自锁。
        if self_name.as_deref() == Some(name.as_str()) {
            return Ok(Err("插件不能卸载自身".to_string()));
        }

        plugin
            .store
            .pump_reentry(&mut host, async move {
                server
                    .plugin_manager
                    .unload_plugin(&name)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await
    }

    async fn enable_plugin(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitPluginManager>,
        name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        plugin
            .store
            .pump_reentry(&mut host, async move {
                server
                    .plugin_manager
                    .enable_plugin(&name)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await
    }

    async fn disable_plugin(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitPluginManager>,
        name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let (server, plugin, self_name) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin, state.name.clone())
        };

        // 与 unload 同理：禁用自身会把 on_disable 钩子回压给正阻塞
        // 在本次宿主调用中的本插件存储驱动器，必然自锁。
        if self_name.as_deref() == Some(name.as_str()) {
            return Ok(Err("插件不能禁用自身".to_string()));
        }

        plugin
            .store
            .pump_reentry(&mut host, async move {
                server
                    .plugin_manager
                    .disable_plugin(&name)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await
    }
}

/// 由元数据与当前状态拼出 WIT 插件信息条目。
async fn build_info(
    manager: &crate::plugin::PluginManager,
    metadata: PluginMetadata,
    active: &HashSet<String>,
) -> WitPluginInfo {
    let (state, detail) = match manager.get_plugin_state(&metadata.name).await {
        Some(PluginState::Loading) => (WitPluginState::Loading, None),
        Some(PluginState::Disabled(reason)) => (WitPluginState::Disabled, Some(reason)),
        Some(PluginState::Failed(reason)) => (WitPluginState::Failed, Some(reason)),
        Some(PluginState::Loaded) | None => (WitPluginState::Loaded, None),
    };
    WitPluginInfo {
        is_active: active.contains(&metadata.name),
        name: metadata.name,
        version: metadata.version,
        description: metadata.description,
        authors: metadata.authors,
        state,
        state_detail: detail,
    }
}
