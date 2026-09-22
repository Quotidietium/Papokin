use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState,
    wit::v0_1::papokin::{
        self,
        plugin::ipc::{IpcMessage, PluginId},
    },
};
use wasmtime::component::{Access, HasSelf};

impl papokin::plugin::ipc::Host for PluginHostState {}

impl papokin::plugin::ipc::HostWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn send_ipc_message(
        mut host: Access<'_, PluginHostState, Self>,
        recipient: PluginId,
        message: IpcMessage,
    ) -> wasmtime::Result<Result<Result<IpcMessage, String>, ()>> {
        let (server, name, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let name = state
                .name
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("插件名称不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, name, plugin)
        };

        let outbound = server
            .plugin_manager
            .send_message(&name, &recipient, &message);
        plugin.store.pump_reentry(&mut host, outbound).await
    }
}
