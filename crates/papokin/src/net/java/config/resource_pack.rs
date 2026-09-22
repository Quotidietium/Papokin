#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub async fn handle_resource_pack_response(
        &self,
        server: &Server,
        packet: SConfigResourcePack,
    ) {
        let resource_config = &server.advanced_config.resource_pack.java;
        if resource_config.enabled {
            let expected_uuid =
                uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, resource_config.url.as_bytes());

            if packet.uuid == expected_uuid {
                match packet.response_result() {
                    ResourcePackResponseResult::DownloadSuccess => {
                        trace!("客户端 {} 成功下载了资源包", self.id);
                    }
                    ResourcePackResponseResult::DownloadFail => {
                        warn!(
                            "客户端 {} 下载资源包失败。资源包是否在公网上可用？",
                            self.id
                        );
                    }
                    ResourcePackResponseResult::Downloaded => {
                        trace!("客户端 {} 已有该资源包", self.id);
                    }
                    ResourcePackResponseResult::Accepted => {
                        trace!("客户端 {} 已接受资源包", self.id);

                        // 在此返回以等待下一次响应更新
                        return;
                    }
                    ResourcePackResponseResult::Declined => {
                        trace!("客户端 {} 拒绝了资源包", self.id);
                    }
                    ResourcePackResponseResult::InvalidUrl => {
                        warn!("客户端 {} 报告资源包 URL 无效！", self.id);
                    }
                    ResourcePackResponseResult::ReloadFailed => {
                        trace!("客户端 {} 重载资源包失败", self.id);
                    }
                    ResourcePackResponseResult::Discarded => {
                        trace!("客户端 {} 丢弃了资源包", self.id);
                    }
                    ResourcePackResponseResult::Unknown(result) => {
                        warn!("客户端 {} 返回了无效的结果：{}！", self.id, result);
                    }
                }
            } else {
                warn!("客户端 {} 返回了我们未设置的资源包的响应！", self.id);
            }
        } else {
            warn!("客户端 {} 返回了未启用的资源包的响应！", self.id);
        }
        self.send_known_packs(server).await;
    }

    pub async fn send_known_packs(&self, server: &Server) {
        let features = server.get_enabled_features();
        self.send_packet(&CFeatureFlags::new(&features)).await;
        let version_str = self.version.load().to_string();
        let loaded_packs = server.datapack_manager.get_loaded_packs();
        let known_packs = server.get_known_packs(&version_str, &loaded_packs);
        self.send_packet(&CKnownPacks::new(&known_packs)).await;
    }
}
