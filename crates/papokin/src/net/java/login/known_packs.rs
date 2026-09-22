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
            let mut registry = papokin_data::registry::Registry::get_synced(version);
            // 在原版条目之后追加插件注册的自定义条目。
            inject_custom_entries(&mut registry, &server.registry_manager);
            for reg in &registry {
                self.send_packet_now(&CRegistryData::new(&reg.registry_id, &reg.registry_entries))
                    .await;
            }
        }
        let tags = server.tag_manager.network_tag_keys(version);
        // 将插件标签覆盖层合并到静态表中（None 当无
        // 插件修改过标签，保留静态表序列化）。
        let merged_tags = server.tag_manager.snapshot(version);
        self.send_packet_now(&CUpdateTags::with_merged(&tags, merged_tags.as_ref()))
            .await;
        self.send_packet_now(&CFinishConfig).await;
    }
}
