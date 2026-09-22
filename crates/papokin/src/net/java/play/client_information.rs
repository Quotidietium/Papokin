#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_client_information(
        &self,
        server: &Arc<Server>,
        player: &Arc<Player>,
        client_information: &SClientInformationPlay<'_>,
    ) {
        if let (Ok(main_hand), Ok(chat_mode)) = (
            Hand::try_from(client_information.main_hand.0),
            ChatMode::try_from(client_information.chat_mode.0),
        ) {
            if client_information.view_distance <= 0 {
                self.try_kick(&TextComponent::text("视距不能为零或负数！"));
                return;
            }

            let (update_settings, update_watched, main_hand_changed, locale_changed) = {
                // 1. 加载当前快照
                let current_config = player.config.load();

                // 2. 在覆盖之前计算设置是否变化
                let main_hand_changed = current_config.main_hand != main_hand;
                let locale_changed = current_config.locale != client_information.locale;
                let update_settings =
                    main_hand_changed || current_config.skin_parts != client_information.skin_parts;

                let old_view_distance = current_config.view_distance;
                let new_view_distance_raw = client_information.view_distance as u8;

                let update_watched = if old_view_distance.get() == new_view_distance_raw {
                    false
                } else {
                    debug!(
                        "玩家 {} ({}) 更新了渲染距离：{} -> {}。",
                        player.gameprofile.name, self.id, old_view_distance, new_view_distance_raw
                    );
                    true
                };

                // 3. 构造新配置
                // 如果 view_distance 为 0，我们提前退出（安全保护）
                let Some(new_view_distance) = NonZero::new(new_view_distance_raw) else {
                    return;
                };

                let new_config = PlayerConfig {
                    locale: client_information.locale.to_string(),
                    view_distance: new_view_distance,
                    chat_mode,
                    chat_colors: client_information.chat_colors,
                    skin_parts: client_information.skin_parts,
                    main_hand,
                    text_filtering: client_information.text_filtering,
                    server_listing: client_information.server_listing,
                };

                // 4. 以原子方式将新配置换入玩家
                player.config.store(std::sync::Arc::new(new_config));

                (
                    update_settings,
                    update_watched,
                    main_hand_changed,
                    locale_changed,
                )
            };

            if update_watched {
                chunker::update_position(player);
            }

            if main_hand_changed {
                let mut event = PlayerChangedMainHandEvent::new(player.clone(), main_hand);
                server.plugin_manager.fire_blocking(server, &mut event);
            }

            if locale_changed {
                let mut event = crate::plugin::api::events::player::player_locale_change::PlayerLocaleChangeEvent {
                    player: player.clone(),
                    new_locale: client_information.locale.to_string(),
                    cancelled: false,
                };
                server.plugin_manager.fire_blocking(server, &mut event);
            }

            if update_settings {
                debug!(
                    "玩家 {} ({}) 更新了皮肤。",
                    player.gameprofile.name, self.id,
                );
                player.send_client_information();
            }
        } else {
            self.try_kick(&TextComponent::text("无效的手或聊天类型"));
        }
    }
}
