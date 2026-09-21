#[allow(clippy::wildcard_imports)]
use super::*;

use crate::server::registry::inject_custom_entries;

impl PendingConnection {
    pub async fn handle_known_packs(&mut self, server: &Server) {
        let version = self.version.load();
        if version.supports_configuration_state() {
            if version < JavaMinecraftVersion::V_1_20_5 {
                let features = server.get_enabled_features();
                self.send_packet_now(&CFeatureFlags::new(&features)).await;
            }
            let mut registry = pumpkin_data::registry::Registry::get_synced(version);
            // Append plugin-registered custom entries after the vanilla ones.
            inject_custom_entries(&mut registry, &server.registry_manager);
            for reg in &registry {
                self.send_packet_now(&CRegistryData::new(&reg.registry_id, &reg.registry_entries))
                    .await;
            }
        }
        let tags = server.tag_manager.network_tag_keys(version);
        // Merge the plugin tag overlay into the static tables (None when no
        // plugin touched the tags, keeping the static-table serialization).
        let merged_tags = server.tag_manager.snapshot(version);
        self.send_packet_now(&CUpdateTags::with_merged(&tags, merged_tags.as_ref()))
            .await;
        self.send_packet_now(&CFinishConfig).await;
    }
}
