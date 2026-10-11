#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_recipe_book_change_settings(
        &self,
        server: &Arc<Server>,
        player: &Arc<Player>,
        packet: &SRecipeBookChangeSettings,
    ) {
        // 订阅者先行：无监听时跳过构造（format! String）。
        if server
            .plugin_manager
            .has_handlers::<crate::plugin::api::events::player::player_recipe_book_settings_change::PlayerRecipeBookSettingsChangeEvent>()
        {
            let mut event = crate::plugin::api::events::player::player_recipe_book_settings_change::PlayerRecipeBookSettingsChangeEvent::new(
                player.clone(),
                format!("{:?}", packet.book_type),
                packet.is_open,
                packet.is_filtering,
            );
            server.plugin_manager.fire_blocking(server, &mut event);
        }
    }
}
