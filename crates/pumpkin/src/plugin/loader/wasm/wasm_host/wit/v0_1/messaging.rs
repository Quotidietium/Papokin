use crate::plugin::loader::wasm::wasm_host::{state::PluginHostState, wit::v0_1::pumpkin};

#[allow(clippy::unused_async_trait_impl)]
impl pumpkin::plugin::messaging::Host for PluginHostState {
    async fn register_incoming_channel(
        &mut self,
        channel: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Err("plugin name is not set".to_string()));
        };
        let Some(server) = self.server.as_ref() else {
            return Ok(Err("server is not set".to_string()));
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
            return Ok(Err("server is not set".to_string()));
        };
        let Ok(uuid) = uuid::Uuid::parse_str(&player_uuid) else {
            return Ok(Err(format!("invalid player uuid `{player_uuid}`")));
        };
        let Some(player) = server.get_player_by_uuid(uuid) else {
            return Ok(Err("player is not online".to_string()));
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
