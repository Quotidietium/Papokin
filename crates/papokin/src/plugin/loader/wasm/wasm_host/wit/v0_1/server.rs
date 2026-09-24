use papokin_util::text::TextComponent;
use wasmtime::component::{Access, HasSelf, Resource};

use crate::command::CommandSender;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::enchantments::{
    CustomEnchantment as WitCustomEnchantment, EnchantmentManager as WitEnchantmentManager,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::recipe::RecipeManager as WitRecipeManager;
use papokin::plugin::server::CommandSender as WasmCommandSender;

/// 用 `message` 踢出 `player`，并首先触发可取消的 `PlayerKickEvent`。
fn kick_player(
    player: &std::sync::Arc<crate::entity::player::Player>,
    server: &std::sync::Arc<crate::server::Server>,
    message: &TextComponent,
) {
    if let Some(player_arc) = player.world().get_player_by_uuid(player.gameprofile.id) {
        let mut event = crate::plugin::api::events::player::player_kick::PlayerKickEvent::new(
            player_arc,
            message.clone().to_pretty_console(),
        );
        server.plugin_manager.fire_blocking(server, &mut event);
        if event.cancelled {
            return;
        }
    }
    player.client.try_kick(message);
}

use super::player::{
    from_wit_permission_level, parse_ban_expiry, text_component_from_resource,
    to_wit_permission_level,
};
use crate::data::SaveJSONConfiguration;
use crate::plugin::{
    loader::wasm::wasm_host::{
        state::{PlayerResource, PluginHostState, ServerResource},
        wit::v0_1::papokin::{
            self,
            plugin::{
                damage_types::DamageTypeManager as WitDamageTypeManager,
                datapack::DatapackManager as WitDatapackManager,
                player::{BanIpOptions, BanPlayerOptions, Player},
                registry::RegistryManager as WitRegistryManager,
                server::{
                    BanManager as WitBanManager, BannedIpEntry, BannedPlayerEntry, Difficulty,
                    Dimension, OfflinePlayerInfo, OpEntry, OpManager as WitOpManager, Server,
                    ServerBuildInfo, SysInfo, WhitelistEntry as WitWhitelistEntry,
                    WhitelistManager as WitWhitelistManager,
                },
                tag::TagManager as WitTagManager,
                uuid::Uuid as WitUuid,
            },
        },
        wit::v0_1::uuid::UuidExt,
    },
    permissions,
};

impl PluginHostState {
    fn get_server_res(&self, res: &Resource<Server>) -> wasmtime::Result<&ServerResource> {
        self.resource_table
            .get::<ServerResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }
}

impl papokin::plugin::server::Host for PluginHostState {}

impl papokin::plugin::server::HostServer for PluginHostState {
    async fn get_sys_info(&mut self, _res: Resource<Server>) -> wasmtime::Result<SysInfo> {
        let has_perm = |p: &str| self.permissions.iter().any(|perm| perm == p);

        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();

        let cpu_count = (has_perm(permissions::SYS_INFO) || has_perm(permissions::SYS_INFO_CPU))
            .then(|| sys.cpus().len() as u32);

        let (total_memory, used_memory) =
            if has_perm(permissions::SYS_INFO) || has_perm(permissions::SYS_INFO_RAM) {
                (Some(sys.total_memory()), Some(sys.used_memory()))
            } else {
                (None, None)
            };

        let (os_name, os_version) =
            if has_perm(permissions::SYS_INFO) || has_perm(permissions::SYS_INFO_OS) {
                (sysinfo::System::name(), sysinfo::System::os_version())
            } else {
                (None, None)
            };

        Ok(SysInfo {
            cpu_count,
            total_memory,
            used_memory,
            os_name,
            os_version,
            papokin_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    async fn get_build_info(
        &mut self,
        _res: Resource<Server>,
    ) -> wasmtime::Result<ServerBuildInfo> {
        Ok(ServerBuildInfo {
            brand: crate::server::connection_cache::CachedBranding::BRAND.to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            minecraft_version: papokin_data::packet::CURRENT_MC_VERSION.to_string(),
            plugin_api_version: crate::plugin::PLUGIN_API_VERSION,
        })
    }

    async fn get_difficulty(&mut self, res: Resource<Server>) -> wasmtime::Result<Difficulty> {
        let resource = self.get_server_res(&res)?;

        Ok(match resource.provider.get_difficulty() {
            papokin_util::Difficulty::Peaceful => Difficulty::Peaceful,
            papokin_util::Difficulty::Easy => Difficulty::Easy,
            papokin_util::Difficulty::Normal => Difficulty::Normal,
            papokin_util::Difficulty::Hard => Difficulty::Hard,
        })
    }

    async fn get_player_count(&mut self, _res: Resource<Server>) -> wasmtime::Result<u32> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.get_player_count() as u32)
    }

    async fn get_mspt(&mut self, _res: Resource<Server>) -> wasmtime::Result<f64> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.get_mspt())
    }

    async fn get_tps(&mut self, _res: Resource<Server>) -> wasmtime::Result<f64> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.get_tps())
    }

    async fn get_all_players(
        &mut self,
        _res: Resource<Server>,
    ) -> wasmtime::Result<Vec<Resource<Player>>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server
            .get_all_players()
            .into_iter()
            .map(|player| self.add_player(player).expect("添加玩家资源失败"))
            .collect())
    }

    async fn get_player_by_name(
        &mut self,
        _rep: Resource<Server>,
        name: String,
    ) -> wasmtime::Result<Option<Resource<Player>>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        server
            .get_player_by_name(&name)
            .map(|player| self.add_player(player))
            .transpose()
    }

    async fn get_player_by_uuid(
        &mut self,
        _rep: Resource<Server>,
        id: WitUuid,
    ) -> wasmtime::Result<Option<Resource<Player>>> {
        let uuid = WitUuid::from_wit(&id);

        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        server
            .get_player_by_uuid(uuid)
            .map(|player| self.add_player(player))
            .transpose()
    }

    async fn get_offline_player_by_uuid(
        &mut self,
        _rep: Resource<Server>,
        uuid: String,
    ) -> wasmtime::Result<Option<OfflinePlayerInfo>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        let Ok(uuid) = uuid::Uuid::parse_str(&uuid) else {
            return Ok(None);
        };

        Ok(offline_player_info(server, uuid, None))
    }

    async fn get_offline_player_by_name(
        &mut self,
        _rep: Resource<Server>,
        name: String,
    ) -> wasmtime::Result<Option<OfflinePlayerInfo>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        if let Some(player) = server.get_player_by_name(&name) {
            let uuid = player.gameprofile.id;
            let player_name = player.gameprofile.name.clone();
            return Ok(offline_player_info(server, uuid, Some(player_name)));
        }

        let cached = server
            .data
            .user_cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_by_name(&name)
            .map(|entry| (entry.uuid, entry.name));
        let Some((uuid, cached_name)) = cached else {
            return Ok(None);
        };

        Ok(offline_player_info(server, uuid, Some(cached_name)))
    }

    async fn get_all_worlds(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Vec<Resource<papokin::plugin::world::World>>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server
            .worlds
            .load()
            .iter()
            .map(|world| self.add_world(world.clone()).expect("添加世界资源失败"))
            .collect())
    }

    async fn get_world_by_name(
        &mut self,
        _rep: Resource<Server>,
        name: String,
    ) -> wasmtime::Result<Option<Resource<papokin::plugin::world::World>>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server
            .worlds
            .load()
            .iter()
            .find(|world| world.get_world_name() == name || world.dimension.minecraft_name == name)
            .map(|world| self.add_world(world.clone()).expect("添加世界资源失败")))
    }

    async fn has_world(&mut self, _rep: Resource<Server>, name: String) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server
            .worlds
            .load()
            .iter()
            .any(|world| world.get_world_name() == name || world.dimension.minecraft_name == name))
    }

    async fn get_players_in_world(
        &mut self,
        _rep: Resource<Server>,
        world: Resource<papokin::plugin::world::World>,
    ) -> wasmtime::Result<Vec<Resource<papokin::plugin::player::Player>>> {
        let world_res = self.get_world_res(&world)?;
        let players = world_res.provider.players.load();
        let mut player_resources = Vec::with_capacity(players.len());
        for p in players.iter() {
            let res = self.add_player(p.clone())?;
            player_resources.push(res);
        }
        Ok(player_resources)
    }

    async fn get_player_count_in_world(
        &mut self,
        _rep: Resource<Server>,
        world: Resource<papokin::plugin::world::World>,
    ) -> wasmtime::Result<u32> {
        let world_res = self.get_world_res(&world)?;
        Ok(world_res.provider.players.load().len() as u32)
    }

    async fn delete_message_by_signature(
        &mut self,
        _rep: Resource<Server>,
        signature: Vec<u8>,
    ) -> wasmtime::Result<()> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let packet = papokin_protocol::java::client::play::CDeleteChat::from_signature(&signature);
        server.broadcast_packet_all(&packet);
        Ok(())
    }

    async fn delete_message_by_id(
        &mut self,
        _rep: Resource<Server>,
        signature_id: i32,
    ) -> wasmtime::Result<()> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let packet = papokin_protocol::java::client::play::CDeleteChat::from_cache_id(signature_id);
        server.broadcast_packet_all(&packet);
        Ok(())
    }

    async fn broadcast_tab_list_header_footer(
        &mut self,
        _rep: Resource<Server>,
        header: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
        footer: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<()> {
        let header = text_component_from_resource(self, &header)?;
        let footer = text_component_from_resource(self, &footer)?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        server.broadcast_tab_list_header_footer(&header, &footer);
        Ok(())
    }

    async fn get_max_players(&mut self, _rep: Resource<Server>) -> wasmtime::Result<u32> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.advanced_config.networking.java.max_players)
    }

    async fn is_hardcore(&mut self, _rep: Resource<Server>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.basic_config.hardcore)
    }

    async fn is_online_mode(&mut self, _rep: Resource<Server>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.advanced_config.networking.java.online_mode)
    }

    async fn get_motd(&mut self, _rep: Resource<Server>) -> wasmtime::Result<String> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.advanced_config.networking.java.motd.clone())
    }

    async fn has_whitelist(&mut self, _rep: Resource<Server>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.basic_config.white_list)
    }

    async fn get_allow_nether(&mut self, _rep: Resource<Server>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.basic_config.allow_nether)
    }

    async fn get_allow_end(&mut self, _rep: Resource<Server>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.basic_config.allow_end)
    }

    async fn get_view_distance(&mut self, _rep: Resource<Server>) -> wasmtime::Result<u8> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server.advanced_config.networking.java.view_distance.get())
    }

    async fn get_simulation_distance(&mut self, _rep: Resource<Server>) -> wasmtime::Result<u8> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(server
            .advanced_config
            .networking
            .java
            .simulation_distance
            .get())
    }

    async fn get_default_gamemode(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<papokin::plugin::common::GameMode> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        Ok(super::events::to_wasm_game_mode(
            server.basic_config.default_gamemode,
        ))
    }

    async fn get_recipe_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitRecipeManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_recipe_manager(server.recipe_manager.clone())
    }

    async fn get_op_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitOpManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_op_manager(server.clone())
    }

    async fn get_ban_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitBanManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_ban_manager(server.clone())
    }

    async fn get_whitelist_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitWhitelistManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_whitelist_manager(server.clone())
    }

    async fn get_advancement(
        &mut self,
        _rep: Resource<Server>,
        id: String,
    ) -> wasmtime::Result<Option<papokin::plugin::advancement::AdvancementInfo>> {
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(&id)
        else {
            return Ok(None);
        };
        crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::to_wasm_advancement_info(
            self,
            advancement,
        )
        .map(Some)
    }

    async fn get_all_advancement_ids(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Vec<String>> {
        let ids = papokin_data::Advancement::get_identifier_list()
            .iter()
            .map(ToString::to_string)
            .collect();
        Ok(ids)
    }

    async fn get_enchantment_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitEnchantmentManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_enchantment_manager(server.enchantment_manager.clone())
    }

    async fn get_enchantment(
        &mut self,
        _rep: Resource<Server>,
        id: String,
    ) -> wasmtime::Result<Option<WitCustomEnchantment>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;

        if let Some(entry) = server.enchantment_manager.get(&id).await {
            let description = self.add_text_component(entry.description)?;
            return Ok(Some(WitCustomEnchantment {
                id: entry.id,
                description,
                max_level: entry.max_level,
                anvil_cost: entry.anvil_cost,
                supported_items: entry.supported_items,
                weight: entry.weight,
                slots: entry
                    .slots
                    .iter()
                    .map(super::enchantment::to_wit_slot)
                    .collect(),
                exclusive_set: entry.exclusive_set,
            }));
        }

        if let Some(vanilla) = super::enchantment::find_vanilla_enchantment(&id) {
            let description =
                self.add_text_component(TextComponent::translate(vanilla.description, []))?;
            return Ok(Some(WitCustomEnchantment {
                id: vanilla.name.to_string(),
                description,
                max_level: vanilla.max_level.max(1) as u32,
                anvil_cost: vanilla.anvil_cost,
                supported_items: vanilla
                    .supported_items
                    .0
                    .first()
                    .copied()
                    .unwrap_or("")
                    .to_string(),
                weight: vanilla.weight.max(1) as u32,
                slots: vanilla
                    .slots
                    .iter()
                    .map(super::enchantment::to_wit_slot)
                    .collect(),
                exclusive_set: vanilla.exclusive_set.map_or_else(Vec::new, |tag| {
                    tag.0.iter().map(|s| (*s).to_string()).collect()
                }),
            }));
        }

        Ok(None)
    }

    async fn get_all_enchantment_ids(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Vec<String>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let mut ids = server.enchantment_manager.get_all_ids().await;
        for enc in papokin_data::enchantment::Enchantment::ALL {
            ids.push(enc.name.to_string());
        }
        Ok(ids)
    }

    async fn get_datapack_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitDatapackManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_datapack_manager(server.clone())
    }

    async fn get_damage_type_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitDamageTypeManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_damage_type_manager(server.damage_type_manager.clone())
    }

    async fn get_tag_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitTagManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_tag_manager(server.tag_manager.clone())
    }

    async fn get_registry_manager(
        &mut self,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Resource<WitRegistryManager>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        self.add_registry_manager(server.registry_manager.clone())
    }

    async fn set_server_links(
        &mut self,
        _rep: Resource<Server>,
        links: Vec<papokin::plugin::player::ServerLink>,
    ) -> wasmtime::Result<()> {
        let server = self
            .server
            .clone()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let mut converted = Vec::new();
        for link in links {
            converted.push(super::player::from_wit_server_link(self, link)?);
        }
        let protocol_links: Vec<papokin_protocol::Link<'_>> = converted
            .iter()
            .map(|(label, url)| papokin_protocol::Link::new(label.clone(), url))
            .collect();
        server.broadcast_server_links(&protocol_links);
        Ok(())
    }

    async fn drop(&mut self, rep: Resource<Server>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<ServerResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::server::HostServerWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn create_world(
        mut host: Access<'_, PluginHostState, Self>,
        _rep: Resource<Server>,
        name: String,
        dimension: Dimension,
    ) -> wasmtime::Result<Resource<papokin::plugin::world::World>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        let internal_dim = match dimension {
            Dimension::Overworld => papokin_data::dimension::Dimension::OVERWORLD,
            Dimension::Nether => papokin_data::dimension::Dimension::THE_NETHER,
            Dimension::End => papokin_data::dimension::Dimension::THE_END,
        };
        let world = plugin
            .store
            .pump_blocking(&mut host, move || server.create_world(name, internal_dim))
            .await?;

        host.get()
            .add_world(world)
            .map_err(|_| wasmtime::Error::msg("添加世界资源失败"))
    }

    async fn unload_world(
        mut host: Access<'_, PluginHostState, Self>,
        _rep: Resource<Server>,
        name: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        plugin
            .store
            .pump_reentry(&mut host, server.unload_world(&name))
            .await
    }

    async fn save_all(
        mut host: Access<'_, PluginHostState, Self>,
        _rep: Resource<Server>,
    ) -> wasmtime::Result<Result<(), String>> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        plugin
            .store
            .pump_reentry(&mut host, server.save_all())
            .await
    }

    async fn broadcast(
        mut host: Access<'_, PluginHostState, Self>,
        _rep: Resource<Server>,
        message: String,
    ) -> wasmtime::Result<()> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };
        let message = TextComponent::text(message);
        let sender = TextComponent::text("服务器");

        plugin
            .store
            .pump_blocking(&mut host, move || {
                server.broadcast_message(&message, &sender, 0, None);
            })
            .await
    }

    async fn execute_command(
        mut host: Access<'_, PluginHostState, Self>,
        _rep: Resource<Server>,
        command: String,
        sender: WasmCommandSender,
    ) -> wasmtime::Result<()> {
        let (server, native_sender, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let native_sender = match sender {
                WasmCommandSender::Console => CommandSender::Console,
                WasmCommandSender::Player(player_res) => {
                    let player_resource = state
                        .resource_table
                        .get::<PlayerResource>(&Resource::new_own(player_res.rep()))?;
                    CommandSender::Player(player_resource.provider.clone())
                }
            };
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, native_sender, plugin)
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let dispatcher = server.command_dispatcher.load();
                dispatcher.handle_command(&native_sender.into_source(&server), &command);
            })
            .await
    }
}

impl papokin::plugin::server::HostOpManager for PluginHostState {
    async fn is_op(&mut self, _res: Resource<WitOpManager>, id: WitUuid) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let ops = server
            .data
            .operator_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(ops.get_entry(&uuid).is_some())
    }

    async fn get_op(
        &mut self,
        _res: Resource<WitOpManager>,
        id: WitUuid,
    ) -> wasmtime::Result<Option<OpEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let ops = server
            .data
            .operator_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(ops.get_entry(&uuid).map(|entry| OpEntry {
            uuid: WitUuid::to_wit(&entry.uuid),
            name: entry.name.clone(),
            level: to_wit_permission_level(entry.level),
            bypasses_player_limit: entry.bypasses_player_limit,
        }))
    }

    async fn get_permission_level(
        &mut self,
        _res: Resource<WitOpManager>,
        id: WitUuid,
    ) -> wasmtime::Result<papokin::plugin::permission::PermissionLevel> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let ops = server
            .data
            .operator_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(ops.get_entry(&uuid).map_or(
            papokin::plugin::permission::PermissionLevel::Zero,
            |entry| to_wit_permission_level(entry.level),
        ))
    }

    async fn list_ops(&mut self, _res: Resource<WitOpManager>) -> wasmtime::Result<Vec<OpEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let config = server
            .data
            .operator_config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(config
            .ops
            .iter()
            .map(|entry| OpEntry {
                uuid: WitUuid::to_wit(&entry.uuid),
                name: entry.name.clone(),
                level: to_wit_permission_level(entry.level),
                bypasses_player_limit: entry.bypasses_player_limit,
            })
            .collect())
    }

    async fn drop(&mut self, rep: Resource<WitOpManager>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<crate::plugin::loader::wasm::wasm_host::state::OpManagerResource>(
                Resource::new_own(rep.rep()),
            );
        Ok(())
    }
}

impl papokin::plugin::server::HostOpManagerWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn op_player(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitOpManager>,
        name: String,
        id: WitUuid,
        level: papokin::plugin::permission::PermissionLevel,
        bypasses_player_limit: bool,
    ) -> wasmtime::Result<()> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };
        let uuid = WitUuid::from_wit(&id);
        let internal_level = from_wit_permission_level(level);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut config = server
                    .data
                    .operator_config
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(existing) = config.ops.iter_mut().find(|o| o.uuid == uuid) {
                    existing.level = internal_level;
                    existing.name.clone_from(&name);
                    existing.bypasses_player_limit = bypasses_player_limit;
                } else {
                    let op_entry = papokin_config::op::Op::new(
                        uuid,
                        name,
                        internal_level,
                        bypasses_player_limit,
                    );
                    config.ops.push(op_entry);
                }
                config.save();
                drop(config);

                if let Some(player) = server.get_player_by_uuid(uuid) {
                    let command_dispatcher = server.command_dispatcher.load();
                    player.set_permission_lvl(&server, internal_level, &command_dispatcher);
                }
            })
            .await
    }

    async fn deop_player(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitOpManager>,
        id: WitUuid,
    ) -> wasmtime::Result<bool> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };
        let uuid = WitUuid::from_wit(&id);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let removed = {
                    let mut config = server
                        .data
                        .operator_config
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    config
                        .ops
                        .iter()
                        .position(|o| o.uuid == uuid)
                        .is_some_and(|op_index| {
                            config.ops.remove(op_index);
                            config.save();
                            true
                        })
                };

                if removed && let Some(player) = server.get_player_by_uuid(uuid) {
                    let command_dispatcher = server.command_dispatcher.load();
                    player.set_permission_lvl(
                        &server,
                        papokin_util::PermissionLvl::Zero,
                        &command_dispatcher,
                    );
                }

                removed
            })
            .await
    }
}

impl papokin::plugin::server::HostBanManager for PluginHostState {
    async fn is_player_banned(
        &mut self,
        _res: Resource<WitBanManager>,
        id: WitUuid,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let now = time::OffsetDateTime::now_utc();
        let mut list = server.data.banned_player_list.write().unwrap();
        list.banned_players
            .retain(|entry| entry.expires.is_none_or(|expires| expires > now));
        list.save();
        Ok(list.banned_players.iter().any(|e| e.uuid == uuid))
    }

    async fn get_player_ban(
        &mut self,
        _res: Resource<WitBanManager>,
        id: WitUuid,
    ) -> wasmtime::Result<Option<BannedPlayerEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let now = time::OffsetDateTime::now_utc();
        let mut list = server.data.banned_player_list.write().unwrap();
        list.banned_players
            .retain(|entry| entry.expires.is_none_or(|expires| expires > now));
        list.save();
        Ok(list
            .banned_players
            .iter()
            .find(|e| e.uuid == uuid)
            .map(|e| BannedPlayerEntry {
                uuid: WitUuid::to_wit(&e.uuid),
                name: e.name.clone(),
                created: e
                    .created
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                source: e.source.clone(),
                expires: e.expires.and_then(|exp| {
                    exp.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
                reason: e.reason.clone(),
            }))
    }

    async fn unban_player(
        &mut self,
        _res: Resource<WitBanManager>,
        id: WitUuid,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let mut list = server.data.banned_player_list.write().unwrap();
        Ok(list
            .banned_players
            .iter()
            .position(|e| e.uuid == uuid)
            .is_some_and(|pos| {
                list.banned_players.remove(pos);
                list.save();
                true
            }))
    }

    async fn list_player_bans(
        &mut self,
        _res: Resource<WitBanManager>,
    ) -> wasmtime::Result<Vec<BannedPlayerEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let now = time::OffsetDateTime::now_utc();
        let mut list = server.data.banned_player_list.write().unwrap();
        list.banned_players
            .retain(|entry| entry.expires.is_none_or(|expires| expires > now));
        list.save();
        Ok(list
            .banned_players
            .iter()
            .map(|e| BannedPlayerEntry {
                uuid: WitUuid::to_wit(&e.uuid),
                name: e.name.clone(),
                created: e
                    .created
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                source: e.source.clone(),
                expires: e.expires.and_then(|exp| {
                    exp.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
                reason: e.reason.clone(),
            })
            .collect())
    }

    async fn is_ip_banned(
        &mut self,
        _res: Resource<WitBanManager>,
        ip: String,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let ip_addr: std::net::IpAddr = ip
            .parse()
            .map_err(|_| wasmtime::Error::msg("无效的 IP 地址"))?;
        let now = time::OffsetDateTime::now_utc();
        let mut list = server.data.banned_ip_list.write().unwrap();
        list.banned_ips
            .retain(|entry| entry.expires.is_none_or(|expires| expires > now));
        list.save();
        Ok(list.banned_ips.iter().any(|e| e.ip == ip_addr))
    }

    async fn get_ip_ban(
        &mut self,
        _res: Resource<WitBanManager>,
        ip: String,
    ) -> wasmtime::Result<Option<BannedIpEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let ip_addr: std::net::IpAddr = ip
            .parse()
            .map_err(|_| wasmtime::Error::msg("无效的 IP 地址"))?;
        let now = time::OffsetDateTime::now_utc();
        let mut list = server.data.banned_ip_list.write().unwrap();
        list.banned_ips
            .retain(|entry| entry.expires.is_none_or(|expires| expires > now));
        list.save();
        Ok(list
            .banned_ips
            .iter()
            .find(|e| e.ip == ip_addr)
            .map(|e| BannedIpEntry {
                ip: e.ip.to_string(),
                created: e
                    .created
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                source: e.source.clone(),
                expires: e.expires.and_then(|exp| {
                    exp.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
                reason: e.reason.clone(),
            }))
    }

    async fn unban_ip(
        &mut self,
        _res: Resource<WitBanManager>,
        ip: String,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let ip_addr: std::net::IpAddr = ip
            .parse()
            .map_err(|_| wasmtime::Error::msg("无效的 IP 地址"))?;
        let mut list = server.data.banned_ip_list.write().unwrap();
        Ok(list
            .banned_ips
            .iter()
            .position(|e| e.ip == ip_addr)
            .is_some_and(|pos| {
                list.banned_ips.remove(pos);
                list.save();
                true
            }))
    }

    async fn list_ip_bans(
        &mut self,
        _res: Resource<WitBanManager>,
    ) -> wasmtime::Result<Vec<BannedIpEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let now = time::OffsetDateTime::now_utc();
        let mut list = server.data.banned_ip_list.write().unwrap();
        list.banned_ips
            .retain(|entry| entry.expires.is_none_or(|expires| expires > now));
        list.save();
        Ok(list
            .banned_ips
            .iter()
            .map(|e| BannedIpEntry {
                ip: e.ip.to_string(),
                created: e
                    .created
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                source: e.source.clone(),
                expires: e.expires.and_then(|exp| {
                    exp.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                }),
                reason: e.reason.clone(),
            })
            .collect())
    }

    async fn drop(&mut self, rep: Resource<WitBanManager>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<crate::plugin::loader::wasm::wasm_host::state::BanManagerResource>(
            Resource::new_own(rep.rep()),
        );
        Ok(())
    }
}

impl papokin::plugin::server::HostBanManagerWithStore<PluginHostState>
    for HasSelf<PluginHostState>
{
    async fn ban_player(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitBanManager>,
        name: String,
        id: WitUuid,
        options: BanPlayerOptions,
    ) -> wasmtime::Result<()> {
        let (server, reason_text, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let reason_text = options
                .reason
                .as_ref()
                .map(|res| text_component_from_resource(state, res))
                .transpose()?
                .map_or_else(
                    || "被插件封禁。".to_string(),
                    papokin_util::text::TextComponent::to_pretty_console,
                );
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, reason_text, plugin)
        };
        let uuid = WitUuid::from_wit(&id);
        let source_name = options.source.unwrap_or_else(|| "Plugin".to_string());
        let expires = parse_ban_expiry(options.expires_at_utc, options.duration_seconds);
        let kick_if_online = options.kick_if_online;
        let log_to_console = options.log_to_console;

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut list = server.data.banned_player_list.write().unwrap();
                if let Some(existing) = list.banned_players.iter_mut().find(|e| e.uuid == uuid) {
                    existing.name.clone_from(&name);
                    existing.source = source_name;
                    existing.expires = expires;
                    existing.reason.clone_from(&reason_text);
                } else {
                    let entry = crate::data::banlist_serializer::BannedPlayerEntry {
                        uuid,
                        name: name.clone(),
                        created: time::OffsetDateTime::now_utc(),
                        source: source_name,
                        expires,
                        reason: reason_text.clone(),
                    };
                    list.banned_players.push(entry);
                }
                list.save();
                drop(list);

                if kick_if_online && let Some(player) = server.get_player_by_uuid(uuid) {
                    kick_player(
                        &player,
                        &server,
                        &papokin_util::text::TextComponent::text(reason_text.clone()),
                    );
                }

                if log_to_console {
                    tracing::info!("已封禁玩家 {}（{}）：{}", name, uuid, reason_text);
                }
            })
            .await
    }

    async fn ban_ip(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitBanManager>,
        ip: String,
        options: BanIpOptions,
    ) -> wasmtime::Result<()> {
        let server = {
            let state = host.get();
            state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?
        };
        let ip_addr: std::net::IpAddr = ip
            .parse()
            .map_err(|_| wasmtime::Error::msg("无效的 IP 地址"))?;
        let (reason_text, plugin) = {
            let state = host.get();
            let reason_text = options
                .reason
                .as_ref()
                .map(|res| text_component_from_resource(state, res))
                .transpose()?
                .map_or_else(
                    || "被插件封禁。".to_string(),
                    papokin_util::text::TextComponent::to_pretty_console,
                );
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (reason_text, plugin)
        };
        let source_name = options.source.unwrap_or_else(|| "Plugin".to_string());
        let expires = parse_ban_expiry(options.expires_at_utc, options.duration_seconds);
        let kick_matching_players = options.kick_matching_players;
        let log_to_console = options.log_to_console;

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut list = server.data.banned_ip_list.write().unwrap();
                if let Some(existing) = list.banned_ips.iter_mut().find(|e| e.ip == ip_addr) {
                    existing.source = source_name;
                    existing.expires = expires;
                    existing.reason.clone_from(&reason_text);
                } else {
                    let entry = crate::data::banlist_serializer::BannedIpEntry {
                        ip: ip_addr,
                        created: time::OffsetDateTime::now_utc(),
                        source: source_name,
                        expires,
                        reason: reason_text.clone(),
                    };
                    list.banned_ips.push(entry);
                }
                list.save();
                drop(list);

                if kick_matching_players {
                    for player in server.get_all_players() {
                        if player.client.address.ip() == ip_addr {
                            kick_player(
                                &player,
                                &server,
                                &papokin_util::text::TextComponent::text(reason_text.clone()),
                            );
                        }
                    }
                }

                if log_to_console {
                    tracing::info!("已封禁 IP {}：{}", ip_addr, reason_text);
                }
            })
            .await
    }
}

impl papokin::plugin::server::HostWhitelistManager for PluginHostState {
    async fn is_enabled(&mut self, _res: Resource<WitWhitelistManager>) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        Ok(server.white_list.load(std::sync::atomic::Ordering::Relaxed))
    }

    async fn is_whitelisted(
        &mut self,
        _res: Resource<WitWhitelistManager>,
        id: WitUuid,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let whitelist = server.data.whitelist_config.read().unwrap();
        Ok(whitelist.whitelist.iter().any(|e| e.uuid == uuid))
    }

    async fn add_player(
        &mut self,
        _res: Resource<WitWhitelistManager>,
        name: String,
        id: WitUuid,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let mut config = server.data.whitelist_config.write().unwrap();
        if config.whitelist.iter().any(|e| e.uuid == uuid) {
            Ok(false)
        } else {
            config
                .whitelist
                .push(papokin_config::whitelist::WhitelistEntry::new(uuid, name));
            config.save();
            Ok(true)
        }
    }

    async fn remove_player(
        &mut self,
        _res: Resource<WitWhitelistManager>,
        id: WitUuid,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let uuid = WitUuid::from_wit(&id);
        let mut config = server.data.whitelist_config.write().unwrap();
        Ok(config
            .whitelist
            .iter()
            .position(|e| e.uuid == uuid)
            .is_some_and(|pos| {
                config.whitelist.remove(pos);
                config.save();
                true
            }))
    }

    async fn list_entries(
        &mut self,
        _res: Resource<WitWhitelistManager>,
    ) -> wasmtime::Result<Vec<WitWhitelistEntry>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
        let config = server.data.whitelist_config.read().unwrap();
        Ok(config
            .whitelist
            .iter()
            .map(|e| WitWhitelistEntry {
                uuid: WitUuid::to_wit(&e.uuid),
                name: e.name.clone(),
            })
            .collect())
    }

    async fn drop(&mut self, rep: Resource<WitWhitelistManager>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<crate::plugin::loader::wasm::wasm_host::state::WhitelistManagerResource>(
            Resource::new_own(rep.rep()),
        );
        Ok(())
    }
}

impl papokin::plugin::server::HostWhitelistManagerWithStore<PluginHostState>
    for HasSelf<PluginHostState>
{
    async fn set_enabled(
        mut host: Access<'_, PluginHostState, Self>,
        _res: Resource<WitWhitelistManager>,
        enabled: bool,
    ) -> wasmtime::Result<()> {
        let (server, plugin) = {
            let state = host.get();
            let server = state
                .server
                .clone()
                .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (server, plugin)
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                server
                    .white_list
                    .store(enabled, std::sync::atomic::Ordering::Relaxed);
                if enabled && server.basic_config.enforce_whitelist {
                    let to_kick: Vec<_> = {
                        let whitelist = server.data.whitelist_config.read().unwrap();
                        server
                            .get_all_players()
                            .into_iter()
                            .filter(|player| !whitelist.is_whitelisted(&player.gameprofile))
                            .collect()
                    };
                    for player in to_kick {
                        kick_player(
                            &player,
                            &server,
                            &papokin_macros::translate!(
                                papokin_data::translation::java::MULTIPLAYER_DISCONNECT_NOT_WHITELISTED
                            ),
                        );
                    }
                }
            })
            .await
    }
}

/// 玩家 UUID 的操作员、白名单和封禁列表归属情况。
struct PlayerListFlags {
    is_op: bool,
    is_whitelisted: bool,
    is_banned: bool,
}

/// 读取 `uuid` 的 OP、白名单与封禁名单成员资格。
fn player_list_flags(server: &crate::server::Server, uuid: &uuid::Uuid) -> PlayerListFlags {
    let is_op = server
        .data
        .operator_config
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_entry(uuid)
        .is_some();
    let is_whitelisted = server
        .data
        .whitelist_config
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .whitelist
        .iter()
        .any(|entry| entry.uuid == *uuid);
    let now = time::OffsetDateTime::now_utc();
    let is_banned = server
        .data
        .banned_player_list
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .banned_players
        .iter()
        .any(|entry| entry.uuid == *uuid && entry.expires.is_none_or(|expires| expires > now));
    PlayerListFlags {
        is_op,
        is_whitelisted,
        is_banned,
    }
}

/// 为当前在线的玩家构建快照（实时值）。
fn online_player_info(
    player: &crate::entity::player::Player,
    uuid: uuid::Uuid,
    flags: &PlayerListFlags,
) -> OfflinePlayerInfo {
    let position = player.living_entity.entity.pos.load();
    let respawn = player
        .respawn_point
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    OfflinePlayerInfo {
        uuid: uuid.to_string(),
        name: Some(player.gameprofile.name.clone()),
        has_played_before: true,
        last_played_ms: None,
        is_online: true,
        last_world: Some(player.world().dimension.minecraft_name.to_string()),
        last_position: Some(super::events::to_wasm_position(position)),
        gamemode: Some(super::events::to_wasm_game_mode(player.gamemode.load())),
        respawn_world: respawn
            .as_ref()
            .map(|point| point.dimension.minecraft_name.to_string()),
        respawn_position: respawn.as_ref().map(|point| {
            (
                f64::from(point.position.0.x),
                f64::from(point.position.0.y),
                f64::from(point.position.0.z),
            )
        }),
        is_op: flags.is_op,
        is_whitelisted: flags.is_whitelisted,
        is_banned: flags.is_banned,
    }
}

/// 为 `uuid` 构建 [`OfflinePlayerInfo`] 快照，优先使用实时数据
/// 先查找在线玩家，再回退到磁盘上的玩家数据文件。
///
///当玩家完全未知时返回 `None`：不在线、没有相关记录
/// 玩家数据文件，也没有用户缓存条目。
fn offline_player_info(
    server: &crate::server::Server,
    uuid: uuid::Uuid,
    cached_name: Option<String>,
) -> Option<OfflinePlayerInfo> {
    let flags = player_list_flags(server, &uuid);
    if let Some(player) = server.get_player_by_uuid(uuid) {
        return Some(online_player_info(&player, uuid, &flags));
    }

    // 玩家数据文件位于 `<world>/players/data/<uuid>.dat`
    // （见 `server/mod.rs` 中的 `ServerPlayerData::new`）。
    let data_file = server
        .basic_config
        .get_world_path()
        .join("players")
        .join("data")
        .join(format!("{uuid}.dat"));
    let has_data_file = data_file.is_file();

    let name = cached_name.or_else(|| {
        server
            .data
            .user_cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_by_uuid(uuid)
            .map(|entry| entry.name)
    });

    if !has_data_file && name.is_none() {
        return None;
    }

    let last_played_ms = std::fs::metadata(&data_file)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_millis() as u64);

    let nbt = if has_data_file {
        match server.player_data_storage.load_data(&uuid) {
            Ok(data) => data,
            Err(err) => {
                tracing::warn!("加载离线玩家 {uuid} 的数据失败：{err}");
                None
            }
        }
    } else {
        None
    };

    let (last_world, last_position, gamemode, respawn_world, respawn_position) =
        nbt.as_ref().map_or_else(
            || (None, None, None, None, None),
            |nbt| {
                let (respawn_world, respawn_position) = read_respawn_point(nbt);
                (
                    nbt.get_string("Dimension").map(str::to_string),
                    read_last_position(nbt),
                    read_gamemode(nbt),
                    respawn_world,
                    respawn_position,
                )
            },
        );

    Some(OfflinePlayerInfo {
        uuid: uuid.to_string(),
        name,
        has_played_before: has_data_file,
        last_played_ms,
        is_online: false,
        last_world,
        last_position,
        gamemode,
        respawn_world,
        respawn_position,
        is_op: flags.is_op,
        is_whitelisted: flags.is_whitelisted,
        is_banned: flags.is_banned,
    })
}

/// 从玩家数据复合标签中读取最后已知的位置（`Pos` 双精度浮点数列表）。
fn read_last_position(
    nbt: &papokin_nbt::compound::NbtCompound,
) -> Option<papokin::plugin::common::Position> {
    let pos = nbt.get_list("Pos")?;
    if pos.len() < 3 {
        return None;
    }
    match (
        pos[0].extract_double(),
        pos[1].extract_double(),
        pos[2].extract_double(),
    ) {
        (Some(x), Some(y), Some(z)) => Some((x, y, z)),
        _ => None,
    }
}

/// 从玩家数据复合标签中读取最后已知的游戏模式（`playerGameType` 整型）。
fn read_gamemode(
    nbt: &papokin_nbt::compound::NbtCompound,
) -> Option<papokin::plugin::common::GameMode> {
    nbt.get_int("playerGameType")
        .and_then(|raw| papokin_util::gamemode::GameMode::try_from(raw).ok())
        .map(super::events::to_wasm_game_mode)
}

/// 从玩家数据复合标签中读取重生点，同时接受
/// 旧版 `SpawnX`/`SpawnY`/`SpawnZ` + `SpawnDimension` 字段以及
/// 原版的 `respawn` 复合标签（`dimension` + `pos` 整型数组）。
fn read_respawn_point(
    nbt: &papokin_nbt::compound::NbtCompound,
) -> (Option<String>, Option<papokin::plugin::common::Position>) {
    if let (Some(x), Some(y), Some(z)) = (
        nbt.get_int("SpawnX"),
        nbt.get_int("SpawnY"),
        nbt.get_int("SpawnZ"),
    ) {
        let world = nbt.get_string("SpawnDimension").map(str::to_string);
        return (world, Some((f64::from(x), f64::from(y), f64::from(z))));
    }

    if let Some(respawn) = nbt.get_compound("respawn")
        && let Some(pos) = respawn.get_int_array("pos")
        && pos.len() >= 3
    {
        let world = respawn.get_string("dimension").map(str::to_string);
        return (
            world,
            Some((f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]))),
        );
    }

    (None, None)
}
