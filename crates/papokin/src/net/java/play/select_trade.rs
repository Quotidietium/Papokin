#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_select_trade(&self, player: &Arc<Player>, packet: &SSelectTrade) {
        let cancelled = if let Some(server) = player
            .world()
            .server
            .upgrade()
            && server
                .plugin_manager
                .has_handlers::<crate::plugin::api::events::inventory::trade_select::TradeSelectEvent>()
        {
            let mut event =
                crate::plugin::api::events::inventory::trade_select::TradeSelectEvent::new(
                    player.clone(),
                    packet.selected_slot.0 as u8,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            event.cancelled
        } else {
            false
        };
        if cancelled {
            return;
        }

        let screen_handler = player
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut screen_handler = screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !screen_handler.can_use(player.as_ref()) {
            return;
        }
        if let Some(merchant) = screen_handler
            .as_any_mut()
            .downcast_mut::<MerchantScreenHandler>()
        {
            merchant.set_selected_offer(packet.selected_slot.0 as usize);
        }
    }
}
