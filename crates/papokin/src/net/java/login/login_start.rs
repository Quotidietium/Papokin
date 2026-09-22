#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub async fn handle_login_start(
        &mut self,
        server: &Arc<Server>,
        login_start: SLoginStart,
    ) -> Option<PacketHandlerResult> {
        debug!("登录开始");

        let max_players = server.advanced_config.networking.java.max_players;
        if max_players > 0 && server.get_player_count() >= max_players as usize {
            // 全服务器钩子：插件仍可允许加入。该事件
            // 仅在服务器真正满员时才触发。
            let mut full_check = crate::plugin::api::events::player::player_server_full_check::PlayerServerFullCheckEvent::new(
                login_start.name.to_string(),
                login_start.uuid,
            );
            server.plugin_manager.fire(server, &mut full_check).await;
            if full_check.result
                == crate::plugin::api::events::player::player_server_full_check::ServerFullCheckResult::Denied
            {
                self.kick(TextComponent::translate(
                    translation::java::MULTIPLAYER_DISCONNECT_SERVER_FULL,
                    []))
                .await;
                return Some(PacketHandlerResult::Stop);
            }
        }

        if !is_valid_player_name(&login_start.name) {
            self.kick(TextComponent::text("用户名包含无效字符")).await;
            return Some(PacketHandlerResult::Stop);
        }

        let proxy = &server.advanced_config.networking.proxy;
        if proxy.enabled {
            if proxy.vine.enabled {
                if self.version.load().is_modern() {
                    vine::vine_login(self).await;
                    None
                } else {
                    self.kick(TextComponent::text("低于 1.13 的客户端版本不支持现代转发"))
                        .await;
                    Some(PacketHandlerResult::Stop)
                }
            } else if proxy.velocity.enabled {
                if self.version.load().is_modern() {
                    velocity::velocity_login(self).await;
                    None
                } else {
                    self.kick(TextComponent::text("低于 1.13 的客户端版本不支持现代转发"))
                        .await;
                    Some(PacketHandlerResult::Stop)
                }
            } else if proxy.bungeecord.enabled {
                match bungeecord::bungeecord_login(
                    &self.address,
                    &self.server_address,
                    login_start.name.into_string(),
                    &proxy.bungeecord.secret,
                ) {
                    Ok((_ip, profile)) => {
                        self.gameprofile = Some(profile.clone());
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
        } else {
            let id = if server.advanced_config.networking.java.online_mode {
                login_start.uuid
            } else {
                offline_uuid(&login_start.name).unwrap_or_else(|_| uuid::Uuid::nil())
            };

            self.handle_login_start_direct(server, login_start.name.into_string(), id)
                .await
        }
    }

    /// 非代理登录路径：插件登录前检查、档案构建，
    /// 然后是加密握手，或者直接完成。
    async fn handle_login_start_direct(
        &mut self,
        server: &Arc<Server>,
        player_name: String,
        id: uuid::Uuid,
    ) -> Option<PacketHandlerResult> {
        let mut pre_login_event = PlayerPreLoginEvent {
            player_name: player_name.clone(),
            player_uuid: id,
            ip_address: self.address,
            kick_message: TextComponent::text("你已被踢出服务器"),
            cancelled: false,
        };
        server
            .plugin_manager
            .fire(server, &mut pre_login_event)
            .await;
        if pre_login_event.cancelled {
            self.kick(pre_login_event.kick_message).await;
            return Some(PacketHandlerResult::Stop);
        }

        let profile = GameProfile {
            id,
            name: player_name,
            properties: ArcSwap::new(Arc::new(vec![])),
            profile_actions: None,
        };

        if server.advanced_config.networking.java.compression.enabled {
            self.enable_compression(server).await;
        }

        self.gameprofile = Some(profile.clone());

        if server.advanced_config.networking.java.encryption {
            let verify_token: [u8; 4] = rand::random();
            self.verify_token = Some(verify_token);
            self.send_packet_now(
                &server
                    .encryption_request(
                        &verify_token,
                        server.advanced_config.networking.java.online_mode,
                    )
                    .await,
            )
            .await;
            None
        } else {
            self.finish_login(server, &profile).await
        }
    }
}
