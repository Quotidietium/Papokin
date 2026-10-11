#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_recipe_book_seen_recipe(
        &self,
        server: &Arc<Server>,
        player: &Arc<Player>,
        packet: &SRecipeBookSeenRecipe,
    ) {
        // 订阅者先行：无监听时跳过构造（format! String）。
        if server
            .plugin_manager
            .has_handlers::<crate::plugin::api::events::player::player_recipe_discover::PlayerRecipeDiscoverEvent>()
        {
            let mut event = crate::plugin::api::events::player::player_recipe_discover::PlayerRecipeDiscoverEvent::new(
                player.clone(),
                format!("display_{}", packet.recipe_display_id.0),
            );
            server.plugin_manager.fire_blocking(server, &mut event);
        }
    }
}
