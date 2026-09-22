#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_bundle_item_selected(&self, player: &Arc<Player>, packet: &SBundleItemSelected) {
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();

        let selected_item_index = packet.selected_item_index.0;
        if selected_item_index < 0 && selected_item_index != -1 {
            self.try_kick(&TextComponent::text("无效的选中物品索引"));
            return;
        }

        debug!(
            "已选择收纳袋物品：槽位 ID {}，选中物品索引 {}",
            packet.slot_id.0, selected_item_index
        );
    }
}
