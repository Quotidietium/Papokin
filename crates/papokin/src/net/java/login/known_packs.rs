#[allow(clippy::wildcard_imports)]
use super::*;

use crate::server::registry::inject_custom_entries;

impl PendingConnection {
    pub async fn handle_known_packs(&mut self, server: &Server) {
        // 每个 Config 会话只允许一次：本函数会全量序列化并发送数百 KB
        // 的注册表/标签数据，改过的客户端重复触发即是出站带宽与 CPU
        // 的资源放大（数十字节的包换数十 MB 的响应）。
        if self
            .known_packs_handled
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            tracing::warn!("连接 {} 重复触发注册表同步，已断开", self.id);
            self.kick(TextComponent::text("重复的已知包响应")).await;
            return;
        }
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
