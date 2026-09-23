#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub async fn handle_plugin_response(
        &mut self,
        server: &Arc<Server>,
        plugin_response: SLoginPluginResponse,
    ) -> Option<PacketHandlerResult> {
        debug!("正在处理插件响应");
        // 事务 id 一次性取出：响应必须与发出的登录插件请求配对。
        let expected_message_id = self.login_plugin_message_id.take();
        let proxy_config = &server.advanced_config.networking.proxy;
        if proxy_config.vine.enabled {
            let expected_challenge = self.vine_challenge.take();
            match vine::receive_vine_plugin_response(
                self.address.port(),
                &proxy_config.vine,
                plugin_response,
                expected_challenge,
                expected_message_id,
            ) {
                Ok((profile, new_address)) => {
                    self.gameprofile = Some(profile.clone());
                    self.address = new_address;
                    self.finish_login(server, &profile).await
                }
                Err(error) => {
                    self.kick(TextComponent::text(error.to_string())).await;
                    Some(PacketHandlerResult::Stop)
                }
            }
        } else if proxy_config.velocity.enabled {
            match velocity::receive_velocity_plugin_response(
                self.address.port(),
                &proxy_config.velocity,
                plugin_response,
                expected_message_id,
            ) {
                Ok((profile, new_address)) => {
                    self.gameprofile = Some(profile.clone());
                    self.address = new_address;
                    self.finish_login(server, &profile).await
                }
                Err(error) => {
                    self.kick(TextComponent::text(error.to_string())).await;
                    Some(PacketHandlerResult::Stop)
                }
            }
        } else {
            None
        }
    }
}
