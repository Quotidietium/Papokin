#[allow(clippy::wildcard_imports)]
use super::*;

use crate::net::is_first_join;
use crate::server::registry::inject_custom_entries;

impl JavaClient {
    pub async fn handle_known_packs(&self, server: &Server) -> Option<PacketHandlerResult> {
        debug!("Handling known packs");

        // The configuration phase starts here; notify plugins asynchronously.
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
                // Append plugin-registered custom entries after the vanilla ones.
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

                // The generated synced-registry table can lag behind the current protocol.
                // ResourceSelectorArgument validates /test names against this registry on the
                // client, so always provide it even when the static table omitted it.
                if !sent_test_instance {
                    let test_instance = "minecraft:test_instance".to_string();
                    let packet = CRegistryData::new(&test_instance, &test_instance_entries);

                    if let Ok(data) = Self::serialize_packet_for_version(&packet, version) {
                        packets.push(data);
                    }
                }

                if !sent_dimension_type {
                    let dims = [
                        &pumpkin_data::dimension::Dimension::OVERWORLD,
                        &pumpkin_data::dimension::Dimension::OVERWORLD_CAVES,
                        &pumpkin_data::dimension::Dimension::THE_END,
                        &pumpkin_data::dimension::Dimension::THE_NETHER,
                    ];

                    let dim_entries: Vec<pumpkin_data::registry::RegistryEntryData> = dims
                        .iter()
                        .map(|dim| pumpkin_data::registry::RegistryEntryData {
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
                // Merge the plugin tag overlay into the static tables (None
                // when no plugin touched the tags).
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

        // We are done with configuring
        self.send_packet(&CFinishConfig).await;

        if !version.supports_configuration_state() {
            return Some(self.handle_config_acknowledged(server).await);
        }

        debug!("Finished config");
        None
    }
}
