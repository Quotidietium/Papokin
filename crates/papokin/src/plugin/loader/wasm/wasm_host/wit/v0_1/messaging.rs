use crate::plugin::loader::wasm::wasm_host::{state::PluginHostState, wit::v0_1::papokin};

#[allow(clippy::unused_async_trait_impl)]
impl papokin::plugin::messaging::Host for PluginHostState {
    async fn register_incoming_channel(
        &mut self,
        channel: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Err("未设置插件名称".to_string()));
        };
        let Some(server) = self.server.as_ref() else {
            return Ok(Err("服务器未设置".to_string()));
        };
        Ok(server
            .plugin_manager
            .register_incoming_channel(&channel, &name))
    }

    async fn unregister_incoming_channel(&mut self, channel: String) -> wasmtime::Result<()> {
        if let (Some(name), Some(server)) = (self.name.clone(), self.server.as_ref()) {
            server
                .plugin_manager
                .unregister_incoming_channel(&channel, &name);
        }
        Ok(())
    }

    async fn get_incoming_channels(&mut self) -> wasmtime::Result<Vec<String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Vec::new());
        };
        let Some(server) = self.server.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(server.plugin_manager.get_plugin_channels(&name))
    }

    async fn send_plugin_message(
        &mut self,
        player_uuid: String,
        channel: String,
        data: Vec<u8>,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(server) = self.server.as_ref() else {
            return Ok(Err("服务器未设置".to_string()));
        };
        let Ok(uuid) = uuid::Uuid::parse_str(&player_uuid) else {
            return Ok(Err(format!("无效的玩家 UUID `{player_uuid}`")));
        };
        let Some(player) = server.get_player_by_uuid(uuid) else {
            return Ok(Err("玩家不在线".to_string()));
        };
        if channel.starts_with("minecraft:") {
            return Ok(Err(format!(
                "channel `{channel}` is reserved (minecraft:*)"
            )));
        }
        crate::entity::player::JavaPlayer(&player)
            .send_custom_payload(&channel, &data)
            .await;
        Ok(Ok(()))
    }
}
