#[allow(clippy::wildcard_imports)]
use super::*;

use crate::net::is_first_join;
use crate::server::registry::inject_custom_entries;

impl JavaClient {
    pub async fn handle_known_packs(&self, server: &Server) -> Option<PacketHandlerResult> {
        debug!("正在处理已知数据包");

        // 配置阶段从这里开始；异步通知插件。
        if let Some(server_arc) = crate::net::server_arc(server) {
            let profile = &self.gameprofile;
            let mut event = crate::plugin::api::events::server::async_player_connection_configure::AsyncPlayerConnectionConfigureEvent::new(
                profile.name.clone(),
                profile.id,
                is_first_join(server, &profile.id),
            );
            server_arc
                .plugin_manager
                .fire(&server_arc, &mut event)
                .await;
        }

        let version = self.version.load();

        if version.supports_configuration_state() {
            if version < JavaMinecraftVersion::V_1_20_5 {
                let features = server.get_enabled_features();
                self.send_packet(&CFeatureFlags::new(&features)).await;
            }

            let test_instance_entries =
                server.datapack_manager.get_test_instance_registry_entries();
            let registry_manager = Arc::clone(&server.registry_manager);
            let tag_manager = Arc::clone(&server.tag_manager);

            let packets = tokio::task::spawn_blocking(move || {
                let mut registry = Registry::get_synced(version);
                // 在原版条目之后追加插件注册的自定义条目。
                inject_custom_entries(&mut registry, &registry_manager);
                let mut packets = Vec::new();
                let mut sent_dimension_type = false;
                let mut sent_test_instance = false;

                for reg in &registry {
                    if reg.registry_id == "minecraft:dimension_type" {
                        sent_dimension_type = true;
                    }

                    if reg.registry_id == "minecraft:test_instance" {
                        sent_test_instance = true;

                        let packet = CRegistryData::new(&reg.registry_id, &test_instance_entries);

                        if let Ok(data) = Self::serialize_packet_for_version(&packet, version) {
                            packets.push(data);
                        }

                        continue;
                    }

                    let packet = CRegistryData::new(&reg.registry_id, &reg.registry_entries);

                    if let Ok(data) = Self::serialize_packet_for_version(&packet, version) {
                        packets.push(data);
                    }
                }

                // 生成的同步注册表可能落后于当前协议。
                // ResourceSelectorArgument 针对此注册表校验 /test 名称，
                // 客户端，因此即使静态表省略了它也始终提供。
                if !sent_test_instance {
                    let test_instance = "minecraft:test_instance".to_string();
                    let packet = CRegistryData::new(&test_instance, &test_instance_entries);

                    if let Ok(data) = Self::serialize_packet_for_version(&packet, version) {
                        packets.push(data);
                    }
                }

                if !sent_dimension_type {
                    let dims = [
                        &papokin_data::dimension::Dimension::OVERWORLD,
                        &papokin_data::dimension::Dimension::OVERWORLD_CAVES,
                        &papokin_data::dimension::Dimension::THE_END,
                        &papokin_data::dimension::Dimension::THE_NETHER,
                    ];

                    let dim_entries: Vec<papokin_data::registry::RegistryEntryData> = dims
                        .iter()
                        .map(|dim| papokin_data::registry::RegistryEntryData {
                            entry_id: dim.minecraft_name.to_string(),
                            data: Some(build_dimension_nbt(dim).into_boxed_slice()),
                        })
                        .collect();

                    let dim_type = "minecraft:dimension_type".to_string();
                    let packet = CRegistryData::new(&dim_type, &dim_entries);

                    if let Ok(data) = Self::serialize_packet_for_version(&packet, version) {
                        packets.push(data);
                    }
                }

                let tags = tag_manager.network_tag_keys(version);
                // 将插件标签覆盖层合并到静态表中（None
                // 当没有插件改动这些标签时）。
                let merged_tags = tag_manager.snapshot(version);
                let packet = CUpdateTags::with_merged(&tags, merged_tags.as_ref());

                if let Ok(data) = Self::serialize_packet_for_version(&packet, version) {
                    packets.push(data);
                }

                packets
            })
            .await
            .unwrap_or_default();

            for packet_data in packets {
                self.send_packet_now(packet_data).await;
            }
        }

        // 配置到此结束
        self.send_packet(&CFinishConfig).await;

        if !version.supports_configuration_state() {
            return Some(self.handle_config_acknowledged(server).await);
        }

        debug!("配置完成");
        None
    }
}
