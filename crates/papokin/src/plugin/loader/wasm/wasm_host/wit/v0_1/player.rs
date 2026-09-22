use std::sync::Arc;
use std::sync::atomic::Ordering;
use wasmtime::component::{Access, HasSelf, Resource};

use crate::plugin::api::gui::PluginScreenHandler;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_dialogs::Dialog;
use crate::{
    entity::{
        EntityBase,
        player::{
            TitleMode,
            advancement::{AdvancementAward, PlayerAdvancement},
        },
    },
    plugin::loader::wasm::wasm_host::{
        DowncastResourceExt, WasmPlugin,
        state::{
            GuiResource, PlayerResource, PluginHostState, TextComponentResource, WorldResource,
        },
        wit::v0_1::{
            events::{
                from_wasm_game_mode, from_wasm_position, to_wasm_game_mode, to_wasm_position,
            },
            living_entity::from_wit_damage_type,
            papokin::{
                self,
                plugin::damage_types::DamageType as WitDamageType,
                plugin::player::{Player, PlayerSkin, SkinParts, TeleportFlags},
                plugin::statistics::{
                    CustomStatistic as WitCustomStatistic,
                    StatisticCategory as WitStatisticCategory,
                },
                plugin::uuid::Uuid,
                plugin::world::World,
            },
            uuid::UuidExt,
            world::from_wit_teleport_flags,
        },
    },
};
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_protocol::Property;
use papokin_protocol::java::client::dialog::DialogNBT;
use papokin_util::permission::PermissionLvl;

use papokin_util::version::JavaMinecraftVersion;

const fn from_wit_client_game_event(
    event: papokin::plugin::player::ClientGameEvent,
) -> papokin_protocol::java::client::play::GameEvent {
    use papokin::plugin::player::ClientGameEvent as W;
    use papokin_protocol::java::client::play::GameEvent as G;
    match event {
        W::NoRespawnBlockAvailable => G::NoRespawnBlockAvailable,
        W::BeginRaining => G::BeginRaining,
        W::EndRaining => G::EndRaining,
        W::ChangeGameMode => G::ChangeGameMode,
        W::WinGame => G::WinGame,
        W::DemoEvent => G::DemoEvent,
        W::ArrowHitPlayer => G::ArrowHitPlayer,
        W::RainLevelChange => G::RainLevelChange,
        W::ThunderLevelChange => G::ThunderLevelChange,
        W::PlayPufferfishStringSound => G::PlayPufferfishStringSound,
        W::PlayElderGuardianMobAppearance => G::PlayElderGuardianMobAppearance,
        W::EnabledRespawnScreen => G::EnabledRespawnScreen,
        W::LimitedCrafting => G::LimitedCrafting,
        W::StartWaitingChunks => G::StartWaitingChunks,
    }
}

pub(crate) fn from_wit_server_link(
    state: &PluginHostState,
    link: papokin::plugin::player::ServerLink,
) -> wasmtime::Result<(papokin_protocol::Label, String)> {
    use papokin::plugin::player::{KnownServerLink, ServerLinkLabel};
    let label = match link.label {
        ServerLinkLabel::Known(known) => {
            let link_type = match known {
                KnownServerLink::BugReport => papokin_protocol::LinkType::BugReport,
                KnownServerLink::CommunityGuidelines => {
                    papokin_protocol::LinkType::CommunityGuidelines
                }
                KnownServerLink::Support => papokin_protocol::LinkType::Support,
                KnownServerLink::Status => papokin_protocol::LinkType::Status,
                KnownServerLink::Feedback => papokin_protocol::LinkType::Feedback,
                KnownServerLink::Community => papokin_protocol::LinkType::Community,
                KnownServerLink::Website => papokin_protocol::LinkType::Website,
                KnownServerLink::Forums => papokin_protocol::LinkType::Forums,
                KnownServerLink::News => papokin_protocol::LinkType::News,
                KnownServerLink::Announcements => papokin_protocol::LinkType::Announcements,
            };
            papokin_protocol::Label::BuiltIn(link_type)
        }
        ServerLinkLabel::Custom(text) => {
            let text_res = state
                .resource_table
                .get::<TextComponentResource>(&Resource::new_own(text.rep()))?;
            papokin_protocol::Label::TextComponent(Box::new(text_res.provider.clone()))
        }
    };
    Ok((label, link.url))
}

const fn to_wasm_java_version(
    version: JavaMinecraftVersion,
) -> papokin::plugin::player::JavaMinecraftVersion {
    match version {
        JavaMinecraftVersion::V_1_7_2 => papokin::plugin::player::JavaMinecraftVersion::V172,
        JavaMinecraftVersion::V_1_7_6 => papokin::plugin::player::JavaMinecraftVersion::V176,
        JavaMinecraftVersion::V_1_8 => papokin::plugin::player::JavaMinecraftVersion::V18,
        JavaMinecraftVersion::V_1_9 => papokin::plugin::player::JavaMinecraftVersion::V19,
        JavaMinecraftVersion::V_1_9_1 => papokin::plugin::player::JavaMinecraftVersion::V191,
        JavaMinecraftVersion::V_1_9_2 => papokin::plugin::player::JavaMinecraftVersion::V192,
        JavaMinecraftVersion::V_1_9_3 => papokin::plugin::player::JavaMinecraftVersion::V193,
        JavaMinecraftVersion::V_1_10 => papokin::plugin::player::JavaMinecraftVersion::V110,
        JavaMinecraftVersion::V_1_11 => papokin::plugin::player::JavaMinecraftVersion::V111,
        JavaMinecraftVersion::V_1_11_1 => papokin::plugin::player::JavaMinecraftVersion::V1111,
        JavaMinecraftVersion::V_1_12 => papokin::plugin::player::JavaMinecraftVersion::V112,
        JavaMinecraftVersion::V_1_12_1 => papokin::plugin::player::JavaMinecraftVersion::V1121,
        JavaMinecraftVersion::V_1_12_2 => papokin::plugin::player::JavaMinecraftVersion::V1122,
        JavaMinecraftVersion::V_1_13 => papokin::plugin::player::JavaMinecraftVersion::V113,
        JavaMinecraftVersion::V_1_13_1 => papokin::plugin::player::JavaMinecraftVersion::V1131,
        JavaMinecraftVersion::V_1_13_2 => papokin::plugin::player::JavaMinecraftVersion::V1132,
        JavaMinecraftVersion::V_1_14 => papokin::plugin::player::JavaMinecraftVersion::V114,
        JavaMinecraftVersion::V_1_14_1 => papokin::plugin::player::JavaMinecraftVersion::V1141,
        JavaMinecraftVersion::V_1_14_2 => papokin::plugin::player::JavaMinecraftVersion::V1142,
        JavaMinecraftVersion::V_1_14_3 => papokin::plugin::player::JavaMinecraftVersion::V1143,
        JavaMinecraftVersion::V_1_14_4 => papokin::plugin::player::JavaMinecraftVersion::V1144,
        JavaMinecraftVersion::V_1_15 => papokin::plugin::player::JavaMinecraftVersion::V115,
        JavaMinecraftVersion::V_1_15_1 => papokin::plugin::player::JavaMinecraftVersion::V1151,
        JavaMinecraftVersion::V_1_15_2 => papokin::plugin::player::JavaMinecraftVersion::V1152,
        JavaMinecraftVersion::V_1_16 => papokin::plugin::player::JavaMinecraftVersion::V116,
        JavaMinecraftVersion::V_1_16_1 => papokin::plugin::player::JavaMinecraftVersion::V1161,
        JavaMinecraftVersion::V_1_16_2 => papokin::plugin::player::JavaMinecraftVersion::V1162,
        JavaMinecraftVersion::V_1_16_3 => papokin::plugin::player::JavaMinecraftVersion::V1163,
        JavaMinecraftVersion::V_1_16_4 => papokin::plugin::player::JavaMinecraftVersion::V1164,
        JavaMinecraftVersion::V_1_17 => papokin::plugin::player::JavaMinecraftVersion::V117,
        JavaMinecraftVersion::V_1_17_1 => papokin::plugin::player::JavaMinecraftVersion::V1171,
        JavaMinecraftVersion::V_1_18 => papokin::plugin::player::JavaMinecraftVersion::V118,
        JavaMinecraftVersion::V_1_18_2 => papokin::plugin::player::JavaMinecraftVersion::V1182,
        JavaMinecraftVersion::V_1_19 => papokin::plugin::player::JavaMinecraftVersion::V119,
        JavaMinecraftVersion::V_1_19_1 => papokin::plugin::player::JavaMinecraftVersion::V1191,
        JavaMinecraftVersion::V_1_19_3 => papokin::plugin::player::JavaMinecraftVersion::V1193,
        JavaMinecraftVersion::V_1_19_4 => papokin::plugin::player::JavaMinecraftVersion::V1194,
        JavaMinecraftVersion::V_1_20 => papokin::plugin::player::JavaMinecraftVersion::V120,
        JavaMinecraftVersion::V_1_20_2 => papokin::plugin::player::JavaMinecraftVersion::V1202,
        JavaMinecraftVersion::V_1_20_3 => papokin::plugin::player::JavaMinecraftVersion::V1203,
        JavaMinecraftVersion::V_1_20_5 => papokin::plugin::player::JavaMinecraftVersion::V1205,
        JavaMinecraftVersion::V_1_21 => papokin::plugin::player::JavaMinecraftVersion::V121,
        JavaMinecraftVersion::V_1_21_2 => papokin::plugin::player::JavaMinecraftVersion::V1212,
        JavaMinecraftVersion::V_1_21_4 => papokin::plugin::player::JavaMinecraftVersion::V1214,
        JavaMinecraftVersion::V_1_21_5 => papokin::plugin::player::JavaMinecraftVersion::V1215,
        JavaMinecraftVersion::V_1_21_6 => papokin::plugin::player::JavaMinecraftVersion::V1216,
        JavaMinecraftVersion::V_1_21_7 => papokin::plugin::player::JavaMinecraftVersion::V1217,
        JavaMinecraftVersion::V_1_21_9 => papokin::plugin::player::JavaMinecraftVersion::V1219,
        JavaMinecraftVersion::V_1_21_11 => papokin::plugin::player::JavaMinecraftVersion::V12111,
        JavaMinecraftVersion::V_26_1 => papokin::plugin::player::JavaMinecraftVersion::V261,
        JavaMinecraftVersion::V_26_2 => papokin::plugin::player::JavaMinecraftVersion::V262,
        JavaMinecraftVersion::V_26_3 => papokin::plugin::player::JavaMinecraftVersion::V263,
        JavaMinecraftVersion::Unknown => papokin::plugin::player::JavaMinecraftVersion::Unknown,
    }
}

const fn to_wasm_chat_mode(
    mode: &crate::entity::player::ChatMode,
) -> papokin::plugin::player::ChatMode {
    match mode {
        crate::entity::player::ChatMode::Enabled => papokin::plugin::player::ChatMode::Enabled,
        crate::entity::player::ChatMode::CommandsOnly => {
            papokin::plugin::player::ChatMode::CommandsOnly
        }
        crate::entity::player::ChatMode::Hidden => papokin::plugin::player::ChatMode::Hidden,
    }
}

#[must_use]
pub const fn to_wit_statistic_category(
    category: papokin_data::statistic::StatisticCategory,
) -> WitStatisticCategory {
    match category {
        papokin_data::statistic::StatisticCategory::Mined => WitStatisticCategory::Mined,
        papokin_data::statistic::StatisticCategory::Crafted => WitStatisticCategory::Crafted,
        papokin_data::statistic::StatisticCategory::Used => WitStatisticCategory::Used,
        papokin_data::statistic::StatisticCategory::Broken => WitStatisticCategory::Broken,
        papokin_data::statistic::StatisticCategory::PickedUp => WitStatisticCategory::PickedUp,
        papokin_data::statistic::StatisticCategory::Dropped => WitStatisticCategory::Dropped,
        papokin_data::statistic::StatisticCategory::Killed => WitStatisticCategory::Killed,
        papokin_data::statistic::StatisticCategory::KilledBy => WitStatisticCategory::KilledBy,
        papokin_data::statistic::StatisticCategory::Custom => WitStatisticCategory::Custom,
    }
}

#[must_use]
pub const fn from_wit_statistic_category(
    wit: WitStatisticCategory,
) -> papokin_data::statistic::StatisticCategory {
    match wit {
        WitStatisticCategory::Mined => papokin_data::statistic::StatisticCategory::Mined,
        WitStatisticCategory::Crafted => papokin_data::statistic::StatisticCategory::Crafted,
        WitStatisticCategory::Used => papokin_data::statistic::StatisticCategory::Used,
        WitStatisticCategory::Broken => papokin_data::statistic::StatisticCategory::Broken,
        WitStatisticCategory::PickedUp => papokin_data::statistic::StatisticCategory::PickedUp,
        WitStatisticCategory::Dropped => papokin_data::statistic::StatisticCategory::Dropped,
        WitStatisticCategory::Killed => papokin_data::statistic::StatisticCategory::Killed,
        WitStatisticCategory::KilledBy => papokin_data::statistic::StatisticCategory::KilledBy,
        WitStatisticCategory::Custom => papokin_data::statistic::StatisticCategory::Custom,
    }
}

#[must_use]
pub const fn to_wit_custom_statistic(
    stat: papokin_data::statistic::CustomStatistic,
) -> WitCustomStatistic {
    // SAFETY: WitCustomStatistic 按与 CustomStatistic 相同的数值顺序生成
    unsafe { std::mem::transmute(stat as u8) }
}

#[must_use]
pub fn from_wit_custom_statistic(
    wit: WitCustomStatistic,
) -> papokin_data::statistic::CustomStatistic {
    papokin_data::statistic::CustomStatistic::from_i32(wit as i32)
        .unwrap_or(papokin_data::statistic::CustomStatistic::PlayTime)
}

pub fn player_from_resource(
    state: &PluginHostState,
    player: &Resource<Player>,
) -> wasmtime::Result<std::sync::Arc<crate::entity::player::Player>> {
    state
        .resource_table
        .get::<PlayerResource>(&Resource::new_own(player.rep()))
        .map_err(|_| wasmtime::Error::msg("无效的玩家资源句柄"))
        .map(|resource| resource.provider.clone())
}

pub(crate) fn text_component_from_resource(
    state: &PluginHostState,
    text: &Resource<papokin::plugin::text::TextComponent>,
) -> papokin_util::text::TextComponent {
    state
        .resource_table
        .get::<TextComponentResource>(&Resource::new_own(text.rep()))
        .expect("无效的文本组件资源句柄")
        .provider
        .clone()
}

fn world_from_resource(
    state: &PluginHostState,
    world: &Resource<papokin::plugin::world::World>,
) -> std::sync::Arc<crate::world::World> {
    state
        .resource_table
        .get::<WorldResource>(&Resource::new_own(world.rep()))
        .expect("无效的世界资源句柄")
        .provider
        .clone()
}

fn plugin_from_state(state: &PluginHostState) -> wasmtime::Result<Arc<WasmPlugin>> {
    state
        .plugin
        .as_ref()
        .and_then(std::sync::Weak::upgrade)
        .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))
}

pub(crate) const fn to_wit_permission_level(
    level: PermissionLvl,
) -> papokin::plugin::permission::PermissionLevel {
    match level {
        PermissionLvl::Zero => papokin::plugin::permission::PermissionLevel::Zero,
        PermissionLvl::One => papokin::plugin::permission::PermissionLevel::One,
        PermissionLvl::Two => papokin::plugin::permission::PermissionLevel::Two,
        PermissionLvl::Three => papokin::plugin::permission::PermissionLevel::Three,
        PermissionLvl::Four => papokin::plugin::permission::PermissionLevel::Four,
    }
}

pub(crate) const fn from_wit_permission_level(
    level: papokin::plugin::permission::PermissionLevel,
) -> PermissionLvl {
    match level {
        papokin::plugin::permission::PermissionLevel::Zero => PermissionLvl::Zero,
        papokin::plugin::permission::PermissionLevel::One => PermissionLvl::One,
        papokin::plugin::permission::PermissionLevel::Two => PermissionLvl::Two,
        papokin::plugin::permission::PermissionLevel::Three => PermissionLvl::Three,
        papokin::plugin::permission::PermissionLevel::Four => PermissionLvl::Four,
    }
}

pub(crate) fn parse_ban_expiry(
    expires_at_utc: Option<String>,
    duration_seconds: Option<u64>,
) -> Option<time::OffsetDateTime> {
    if let Some(dur) = duration_seconds {
        let seconds = i64::try_from(dur).unwrap_or(i64::MAX);
        return Some(time::OffsetDateTime::now_utc() + time::Duration::seconds(seconds));
    }
    if let Some(s) = expires_at_utc {
        if s.eq_ignore_ascii_case("forever") || s.is_empty() {
            return None;
        }
        if let Ok(parsed) =
            time::OffsetDateTime::parse(&s, &time::format_description::well_known::Rfc3339)
        {
            return Some(parsed);
        }
        if let Ok(parsed) = time::OffsetDateTime::parse(
            &s,
            time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second][offset_hour sign:mandatory]:[offset_minute]"
            ),
        ) {
            return Some(parsed);
        }
    }
    None
}

impl DowncastResourceExt<PlayerResource> for Resource<Player> {
    fn downcast_ref<'a>(&'a self, state: &'a mut PluginHostState) -> &'a PlayerResource {
        state
            .resource_table
            .get_any_mut(self.rep())
            .map_err(|_| wasmtime::Error::msg("无效的玩家资源句柄"))
            .expect("有效的玩家资源句柄")
            .downcast_ref::<PlayerResource>()
            .ok_or("资源类型不匹配")
            .map_err(wasmtime::Error::msg)
            .expect("资源类型不匹配")
    }

    fn downcast_mut<'a>(&'a self, state: &'a mut PluginHostState) -> &'a mut PlayerResource {
        state
            .resource_table
            .get_any_mut(self.rep())
            .map_err(|_| wasmtime::Error::msg("无效的玩家资源句柄"))
            .expect("有效的玩家资源句柄")
            .downcast_mut::<PlayerResource>()
            .ok_or("资源类型不匹配")
            .map_err(wasmtime::Error::msg)
            .expect("资源类型不匹配")
    }

    fn consume(self, state: &mut PluginHostState) -> PlayerResource {
        state
            .resource_table
            .delete::<PlayerResource>(Resource::new_own(self.rep()))
            .map_err(|_| wasmtime::Error::msg("无效的玩家资源句柄"))
            .expect("无效的玩家资源句柄")
    }
}

impl papokin::plugin::player::Host for PluginHostState {
    async fn get_world_players(
        &mut self,
        world_ref: Resource<papokin::plugin::world::World>,
    ) -> wasmtime::Result<Vec<Resource<papokin::plugin::player::Player>>> {
        let world = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::WorldResource>(
                &Resource::new_own(world_ref.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的世界资源句柄"))?
            .provider
            .clone();

        let mut players = Vec::new();
        for player in world.players.load().iter() {
            players.push(self.add_player(player.clone())?);
        }

        Ok(players)
    }
}
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::events::from_wasm_hand;
use papokin_inventory::generic_container_screen_handler::GenericContainerScreenHandler;
use papokin_inventory::player::ender_chest_inventory::EnderChestInventory;
use papokin_inventory::{Clearable, Inventory};
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::java::client::play::CSetContainerSlot;

use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::item_stack::ItemStack as WitHostItemStack;

impl papokin::plugin::player::HostPlayer for PluginHostState {
    async fn set_item_in_hand(
        &mut self,
        player: Resource<Player>,
        hand: papokin::plugin::common::Hand,
        stack: Option<Resource<WitHostItemStack>>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let stack = if let Some(stack_res) = stack {
            self.get_item_stack(&stack_res)?.lock().await.clone()
        } else {
            papokin_data::item_stack::ItemStack::EMPTY.clone()
        };

        let hand = from_wasm_hand(hand);
        let slot = match hand {
            papokin_util::Hand::Right => player.inventory().get_selected_slot() as usize,
            papokin_util::Hand::Left => PlayerInventory::OFF_HAND_SLOT,
        };

        player.inventory().set_stack(slot, stack.clone());

        // 同步到客户端
        let stack_serializer = ItemStackSerializer::from(stack);
        let packet = CSetContainerSlot::new(0, 0, slot as i16, &stack_serializer);
        player.send_client_packet(&packet).await;

        Ok(())
    }

    async fn set_inventory_item(
        &mut self,
        player: Resource<Player>,
        slot: u8,
        stack: Option<Resource<WitHostItemStack>>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let stack = if let Some(stack_res) = stack {
            self.get_item_stack(&stack_res)?.lock().await.clone()
        } else {
            papokin_data::item_stack::ItemStack::EMPTY.clone()
        };

        player.inventory().set_stack(slot as usize, stack.clone());

        // 同步到客户端
        let stack_serializer = ItemStackSerializer::from(stack);
        let packet = CSetContainerSlot::new(0, 0, slot as i16, &stack_serializer);
        player.send_client_packet(&packet).await;

        Ok(())
    }

    async fn get_inventory(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<
        Resource<
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::inventory::PlayerInventory,
        >,
    >{
        let player = player_from_resource(self, &player)?;
        self.add_player_inventory(player)
    }

    async fn get_ender_chest(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<
        Resource<
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::inventory::Inventory,
        >,
    >{
        let player = player_from_resource(self, &player)?;
        self.add_inventory(
            crate::plugin::loader::wasm::wasm_host::state::InventoryProvider::PlayerEnderChest(
                player,
            ),
        )
    }

    async fn get_inventory_item(
        &mut self,
        player: Resource<Player>,
        slot: u8,
    ) -> wasmtime::Result<Option<Resource<WitHostItemStack>>> {
        let player = player_from_resource(self, &player)?;
        let stack = player.inventory().get_stack(slot as usize);
        if stack.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.add_item_stack(Arc::new(
                tokio::sync::Mutex::new(stack),
            ))?))
        }
    }

    async fn get_ender_chest_item(
        &mut self,
        player: Resource<Player>,
        slot: u8,
    ) -> wasmtime::Result<Option<Resource<WitHostItemStack>>> {
        let player = player_from_resource(self, &player)?;
        let ec = player.ender_chest_inventory();
        let stack = ec.get_stack(slot as usize);
        if stack.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.add_item_stack(Arc::new(
                tokio::sync::Mutex::new(stack),
            ))?))
        }
    }

    async fn set_ender_chest_item(
        &mut self,
        player: Resource<Player>,
        slot: u8,
        stack: Option<Resource<WitHostItemStack>>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let stack = if let Some(stack_res) = stack {
            self.get_item_stack(&stack_res)?.lock().await.clone()
        } else {
            papokin_data::item_stack::ItemStack::EMPTY.clone()
        };

        let ec = player.ender_chest_inventory();
        ec.set_stack(slot as usize, stack.clone());

        // 如果玩家当前打开了末影箱界面，同步该槽位
        let sync_id = {
            let screen_handler_arc = player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let handler = screen_handler_arc
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(generic) = handler
                .as_any()
                .downcast_ref::<GenericContainerScreenHandler>()
                && generic.inventory.as_any().is::<EnderChestInventory>()
            {
                Some(handler.sync_id())
            } else {
                None
            }
        };
        if let Some(sync_id) = sync_id {
            let stack_serializer = ItemStackSerializer::from(stack);
            let packet = CSetContainerSlot::new(sync_id as i8, 0, slot as i16, &stack_serializer);
            player.send_client_packet(&packet).await;
        }

        Ok(())
    }

    async fn clear_ender_chest(&mut self, player: Resource<Player>) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let ec = player.ender_chest_inventory();
        ec.clear();

        // 如果玩家当前打开了末影箱界面，同步所有槽位
        let sync_id = {
            let screen_handler_arc = player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let handler = screen_handler_arc
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(generic) = handler
                .as_any()
                .downcast_ref::<GenericContainerScreenHandler>()
                && generic.inventory.as_any().is::<EnderChestInventory>()
            {
                Some(handler.sync_id())
            } else {
                None
            }
        };
        if let Some(sync_id) = sync_id {
            let empty_serializer =
                ItemStackSerializer::from(papokin_data::item_stack::ItemStack::EMPTY.clone());
            for slot in 0..27 {
                let packet =
                    CSetContainerSlot::new(sync_id as i8, 0, slot as i16, &empty_serializer);
                player.send_client_packet(&packet).await;
            }
        }

        Ok(())
    }

    async fn get_item_in_hand(
        &mut self,
        player: Resource<Player>,
        hand: papokin::plugin::common::Hand,
    ) -> wasmtime::Result<Option<Resource<WitHostItemStack>>> {
        let player = player_from_resource(self, &player)?;
        let hand = from_wasm_hand(hand);
        let stack = player.inventory().get_stack_in_hand(hand);
        if stack.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.add_item_stack(Arc::new(
                tokio::sync::Mutex::new(stack),
            ))?))
        }
    }

    async fn as_entity(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Resource<papokin::plugin::world::Entity>> {
        let player = player_from_resource(self, &player)?;
        self.add_entity(player as Arc<dyn EntityBase>)
            .map_err(|_| wasmtime::Error::msg("添加实体资源失败"))
    }

    async fn get_id(&mut self, player: Resource<Player>) -> wasmtime::Result<Uuid> {
        let player = player_from_resource(self, &player)?;
        Ok(Uuid::to_wit(&player.gameprofile.id))
    }

    async fn get_name(&mut self, player: Resource<Player>) -> wasmtime::Result<String> {
        let player = player_from_resource(self, &player)?;
        Ok(player.gameprofile.name.clone())
    }

    async fn get_position(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<papokin::plugin::common::Position> {
        let player = player_from_resource(self, &player)?;
        Ok(to_wasm_position(player.position()))
    }

    async fn get_yaw(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_entity().yaw.load())
    }

    async fn get_pitch(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_entity().pitch.load())
    }

    async fn get_world(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<wasmtime::component::Resource<papokin::plugin::world::World>> {
        let player = player_from_resource(self, &player)?;
        let world = player.world();
        self.add_world(world)
            .map_err(|_| wasmtime::Error::msg("添加世界资源失败"))
    }

    async fn get_gamemode(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<papokin::plugin::common::GameMode> {
        let player = player_from_resource(self, &player)?;
        Ok(to_wasm_game_mode(player.gamemode.load()))
    }

    async fn get_locale(&mut self, player: Resource<Player>) -> wasmtime::Result<String> {
        let player = player_from_resource(self, &player)?;
        Ok(player.config.load().locale.clone())
    }

    async fn get_ping(&mut self, player: Resource<Player>) -> wasmtime::Result<u32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.ping.load(Ordering::Relaxed))
    }

    async fn get_permission_level(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<papokin::plugin::permission::PermissionLevel> {
        let player = player_from_resource(self, &player)?;
        Ok(to_wit_permission_level(player.permission_lvl.load()))
    }

    async fn set_permission(
        &mut self,
        player: Resource<Player>,
        node: String,
        value: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let server = self.server.as_ref().expect("服务器不可用");

        server
            .permission_manager
            .set_permission(player.gameprofile.id, node, value);

        Ok(())
    }

    async fn unset_permission(
        &mut self,
        player: Resource<Player>,
        node: String,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let server = self.server.as_ref().expect("服务器不可用");

        server
            .permission_manager
            .unset_permission(&player.gameprofile.id, &node);

        Ok(())
    }

    async fn has_permission_set(
        &mut self,
        player: Resource<Player>,
        node: String,
    ) -> wasmtime::Result<Option<bool>> {
        let player = player_from_resource(self, &player)?;
        let server = self.server.as_ref().expect("服务器不可用");

        Ok(server
            .permission_manager
            .has_permission_set(&player.gameprofile.id, &node))
    }

    async fn get_display_name(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Resource<papokin::plugin::text::TextComponent>> {
        let player = player_from_resource(self, &player)?;
        let display_name = player.get_display_name();
        self.add_text_component(display_name)
            .map_err(|_| wasmtime::Error::msg("添加文本组件资源失败"))
    }

    async fn set_display_name(
        &mut self,
        player: Resource<Player>,
        display_name: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<()> {
        let display_name = text_component_from_resource(self, &display_name);
        let player = player_from_resource(self, &player)?;
        player.set_display_name(Some(display_name));
        Ok(())
    }

    async fn get_tab_list_name(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Option<Resource<papokin::plugin::text::TextComponent>>> {
        let player = player_from_resource(self, &player)?;
        let tab_list_name = player.get_tab_list_name();
        tab_list_name.map_or_else(
            || Ok(None),
            |name| {
                self.add_text_component(name)
                    .map(Some)
                    .map_err(|_| wasmtime::Error::msg("添加文本组件资源失败"))
            },
        )
    }

    async fn set_tab_list_name(
        &mut self,
        player: Resource<Player>,
        name: Option<wasmtime::component::Resource<papokin::plugin::text::TextComponent>>,
    ) -> wasmtime::Result<()> {
        let name = name.map(|n| text_component_from_resource(self, &n));
        let player = player_from_resource(self, &player)?;
        player.set_tab_list_name(name);
        Ok(())
    }

    async fn send_system_message(
        &mut self,
        player: Resource<Player>,
        text: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
        overlay: bool,
    ) -> wasmtime::Result<()> {
        let component = text_component_from_resource(self, &text);
        let player = player_from_resource(self, &player)?;
        player.send_system_message_raw(&component, overlay);
        Ok(())
    }

    async fn delete_message_by_signature(
        &mut self,
        player: Resource<Player>,
        signature: Vec<u8>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let packet = papokin_protocol::java::client::play::CDeleteChat::from_signature(&signature);
        player.client.send_packet(&packet).await;
        Ok(())
    }

    async fn delete_message_by_id(
        &mut self,
        player: Resource<Player>,
        signature_id: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let packet = papokin_protocol::java::client::play::CDeleteChat::from_cache_id(signature_id);
        player.client.send_packet(&packet).await;
        Ok(())
    }

    async fn set_camera(
        &mut self,
        player: Resource<Player>,
        entity: Option<
            Resource<
                crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::world::Entity,
            >,
        >,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        if let Some(target_res) = entity {
            let target =
                crate::plugin::loader::wasm::wasm_host::wit::v0_1::entity::entity_from_resource(
                    self,
                    &target_res,
                )?;
            player.set_camera_entity_id(target.get_entity().entity_id);
        } else {
            player.reset_camera();
        }
        Ok(())
    }

    async fn set_camera_entity_id(
        &mut self,
        player: Resource<Player>,
        entity_id: u32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_camera_entity_id(entity_id as i32);
        Ok(())
    }

    async fn get_camera_entity_id(&mut self, player: Resource<Player>) -> wasmtime::Result<u32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_camera_entity_id() as u32)
    }

    async fn reset_camera(&mut self, player: Resource<Player>) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.reset_camera();
        Ok(())
    }

    async fn play_sound(
        &mut self,
        player: Resource<Player>,
        sound: papokin::plugin::sounds::Sound,
        category: papokin::plugin::sounds::SoundCategory,
        volume: f32,
        pitch: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let sound_name = format!("{sound:?}").to_lowercase().replace('_', ".");
        let sound_data = papokin_data::sound::Sound::from_name(&sound_name)
            .ok_or_else(|| wasmtime::Error::msg(format!("未知的声音：{sound_name}")))?;
        let internal_category = super::world::from_wit_sound_category(category);
        let pos = player.position();
        player.play_sound(
            sound_data as u16,
            internal_category,
            &pos,
            volume,
            pitch,
            rand::random::<i64>(),
        );
        Ok(())
    }

    async fn play_sound_at(
        &mut self,
        player: Resource<Player>,
        pos: papokin::plugin::common::Position,
        sound: papokin::plugin::sounds::Sound,
        category: papokin::plugin::sounds::SoundCategory,
        volume: f32,
        pitch: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let sound_name = format!("{sound:?}").to_lowercase().replace('_', ".");
        let sound_data = papokin_data::sound::Sound::from_name(&sound_name)
            .ok_or_else(|| wasmtime::Error::msg(format!("未知的声音：{sound_name}")))?;
        let internal_category = super::world::from_wit_sound_category(category);
        player.play_sound(
            sound_data as u16,
            internal_category,
            &papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2),
            volume,
            pitch,
            rand::random::<i64>(),
        );
        Ok(())
    }

    async fn stop_sound(
        &mut self,
        player: Resource<Player>,
        sound: Option<papokin::plugin::sounds::Sound>,
        category: Option<papokin::plugin::sounds::SoundCategory>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let sound_rl = sound.and_then(|s| {
            let sound_name = format!("{s:?}").to_lowercase().replace('_', ".");
            papokin_data::sound::Sound::from_name(&sound_name).map(|s| s.to_name().into())
        });
        let cat = category.map(super::world::from_wit_sound_category);
        player.stop_sound(sound_rl, cat);
        Ok(())
    }

    async fn play_custom_sound(
        &mut self,
        player: Resource<Player>,
        sound_name: String,
        category: papokin::plugin::sounds::SoundCategory,
        volume: f32,
        pitch: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let internal_category = super::world::from_wit_sound_category(category);
        let pos = player.position();
        player.play_custom_sound(&sound_name, internal_category, &pos, volume, pitch);
        Ok(())
    }

    async fn play_custom_sound_at(
        &mut self,
        player: Resource<Player>,
        pos: papokin::plugin::common::Position,
        sound_name: String,
        category: papokin::plugin::sounds::SoundCategory,
        volume: f32,
        pitch: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let internal_category = super::world::from_wit_sound_category(category);
        player.play_custom_sound(
            &sound_name,
            internal_category,
            &papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2),
            volume,
            pitch,
        );
        Ok(())
    }

    async fn stop_custom_sound(
        &mut self,
        player: Resource<Player>,
        sound_name: Option<String>,
        category: Option<papokin::plugin::sounds::SoundCategory>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let sound_rl = sound_name
            .as_deref()
            .map(papokin_util::resource_location::ResourceLocation::from);
        let cat = category.map(super::world::from_wit_sound_category);
        player.stop_sound(sound_rl, cat);
        Ok(())
    }

    async fn spawn_particles(
        &mut self,
        player: Resource<Player>,
        particle: papokin::plugin::particles::Particle,
        pos: papokin::plugin::common::Position,
        count: u32,
        offset: papokin::plugin::common::Position,
        max_speed: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let particle_data = papokin_data::particle::Particle::from_id(particle as u16)
            .ok_or_else(|| wasmtime::Error::msg(format!("未知的粒子 ID：{}", particle as u16)))?;
        player.spawn_particles(
            particle_data,
            papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2),
            count,
            papokin_util::math::vector3::Vector3::new(
                offset.0 as f32,
                offset.1 as f32,
                offset.2 as f32,
            ),
            max_speed,
        );
        Ok(())
    }

    async fn send_block_change(
        &mut self,
        player: Resource<Player>,
        pos: papokin::plugin::common::BlockPos,
        block_id: u16,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.send_block_change(
            papokin_util::math::position::BlockPos(papokin_util::math::vector3::Vector3::new(
                pos.x, pos.y, pos.z,
            )),
            block_id,
        );
        Ok(())
    }

    async fn reset_block_change(
        &mut self,
        player: Resource<Player>,
        pos: papokin::plugin::common::BlockPos,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.reset_block_change(papokin_util::math::position::BlockPos(
            papokin_util::math::vector3::Vector3::new(pos.x, pos.y, pos.z),
        ));
        Ok(())
    }

    async fn send_hurt_animation(
        &mut self,
        player: Resource<Player>,
        yaw: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.send_hurt_animation(yaw);
        Ok(())
    }

    async fn open_book(
        &mut self,
        player: Resource<Player>,
        hand: papokin::plugin::common::Hand,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let hand = match hand {
            papokin::plugin::common::Hand::Right => papokin_util::Hand::Right,
            papokin::plugin::common::Hand::Left => papokin_util::Hand::Left,
        };
        player.open_book(hand);
        Ok(())
    }

    async fn open_sign_editor(
        &mut self,
        player: Resource<Player>,
        pos: papokin::plugin::common::BlockPos,
        is_front_text: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.open_sign_editor(
            papokin_util::math::position::BlockPos(papokin_util::math::vector3::Vector3::new(
                pos.x, pos.y, pos.z,
            )),
            is_front_text,
        );
        Ok(())
    }

    async fn set_velocity(
        &mut self,
        player: Resource<Player>,
        velocity: papokin::plugin::common::Position,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_velocity(papokin_util::math::vector3::Vector3::new(
            velocity.0, velocity.1, velocity.2,
        ));
        Ok(())
    }

    async fn apply_knockback(
        &mut self,
        player: Resource<Player>,
        strength: f64,
        x: f64,
        z: f64,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.apply_knockback(strength, x, z);
        Ok(())
    }

    async fn set_movement_locked(
        &mut self,
        player: Resource<Player>,
        locked: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_movement_locked(locked);
        Ok(())
    }

    async fn is_movement_locked(&mut self, player: Resource<Player>) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        Ok(player.is_movement_locked())
    }

    async fn set_freeze_ticks(
        &mut self,
        player: Resource<Player>,
        ticks: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_freeze_ticks(ticks);
        Ok(())
    }

    async fn get_freeze_ticks(&mut self, player: Resource<Player>) -> wasmtime::Result<i32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_freeze_ticks())
    }

    async fn set_server_links(
        &mut self,
        player: Resource<Player>,
        links: Vec<papokin::plugin::player::ServerLink>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let mut converted = Vec::new();
        for link in links {
            converted.push(from_wit_server_link(self, link)?);
        }
        let protocol_links: Vec<papokin_protocol::Link<'_>> = converted
            .iter()
            .map(|(label, url)| papokin_protocol::Link::new(label.clone(), url))
            .collect();
        player.set_server_links(&protocol_links);
        Ok(())
    }

    async fn remove_effect(
        &mut self,
        player: Resource<Player>,
        effect: papokin::plugin::status_effect::StatusEffectType,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let effect_type = super::status_effect::from_wasm_status_effect_type(effect);
        if let Some(status_effect) =
            papokin_data::effect::StatusEffect::from_name(effect_type.to_name())
        {
            player.remove_effect(status_effect);
        }
        Ok(())
    }

    async fn clear_effects(&mut self, player: Resource<Player>) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.remove_all_effects();
        Ok(())
    }

    async fn has_effect(
        &mut self,
        player: Resource<Player>,
        effect: papokin::plugin::status_effect::StatusEffectType,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        let effect_type = super::status_effect::from_wasm_status_effect_type(effect);
        Ok(
            papokin_data::effect::StatusEffect::from_name(effect_type.to_name())
                .is_some_and(|status_effect| player.has_effect(status_effect)),
        )
    }

    async fn get_effect(
        &mut self,
        player: Resource<Player>,
        effect: papokin::plugin::status_effect::StatusEffectType,
    ) -> wasmtime::Result<Option<papokin::plugin::status_effect::StatusEffectInstance>> {
        let player = player_from_resource(self, &player)?;
        let effect_type = super::status_effect::from_wasm_status_effect_type(effect);
        if let Some(status_effect) =
            papokin_data::effect::StatusEffect::from_name(effect_type.to_name())
            && let Some(eff) = player.get_effect(status_effect)
        {
            return Ok(super::status_effect::to_wasm_status_effect_instance(&eff));
        }
        Ok(None)
    }

    async fn get_active_effects(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Vec<papokin::plugin::status_effect::StatusEffectInstance>> {
        let player = player_from_resource(self, &player)?;
        let effects = player.get_active_effects();
        let mut list = Vec::with_capacity(effects.len());
        for eff in &effects {
            if let Some(instance) = super::status_effect::to_wasm_status_effect_instance(eff) {
                list.push(instance);
            }
        }
        Ok(list)
    }

    async fn get_statistic(
        &mut self,
        player: Resource<Player>,
        category: WitStatisticCategory,
        stat_id: i32,
    ) -> wasmtime::Result<i32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_stat(from_wit_statistic_category(category), stat_id))
    }

    async fn set_statistic(
        &mut self,
        player: Resource<Player>,
        category: WitStatisticCategory,
        stat_id: i32,
        value: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_stat(from_wit_statistic_category(category), stat_id, value);
        Ok(())
    }

    async fn increment_statistic(
        &mut self,
        player: Resource<Player>,
        category: WitStatisticCategory,
        stat_id: i32,
        amount: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.increment_stat(from_wit_statistic_category(category), stat_id, amount);
        Ok(())
    }

    async fn get_custom_statistic(
        &mut self,
        player: Resource<Player>,
        stat: WitCustomStatistic,
    ) -> wasmtime::Result<i32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_custom_stat(from_wit_custom_statistic(stat)))
    }

    async fn set_custom_statistic(
        &mut self,
        player: Resource<Player>,
        stat: WitCustomStatistic,
        value: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_custom_stat(from_wit_custom_statistic(stat), value);
        Ok(())
    }

    async fn increment_custom_statistic(
        &mut self,
        player: Resource<Player>,
        stat: WitCustomStatistic,
        amount: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.increment_custom_stat(from_wit_custom_statistic(stat), amount);
        Ok(())
    }

    async fn send_stats(&mut self, player: Resource<Player>) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.send_stats();
        Ok(())
    }

    async fn get_team(&mut self, player: Resource<Player>) -> wasmtime::Result<Option<String>> {
        let player = player_from_resource(self, &player)?;
        let team = player.get_team();
        Ok(team.map(|t| t.name))
    }

    async fn start_cooldown(
        &mut self,
        player: Resource<Player>,
        group: String,
        duration_ticks: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.start_cooldown(group, duration_ticks);
        Ok(())
    }

    async fn get_cooldown(
        &mut self,
        player: Resource<Player>,
        group: String,
    ) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_cooldown(&group))
    }

    async fn is_on_cooldown(
        &mut self,
        player: Resource<Player>,
        group: String,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        Ok(player.is_on_cooldown(&group))
    }

    async fn set_allow_flight(
        &mut self,
        player: Resource<Player>,
        allowed: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_allow_flight(allowed);
        Ok(())
    }

    async fn set_fly_speed(
        &mut self,
        player: Resource<Player>,
        speed: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_fly_speed(speed);
        Ok(())
    }

    async fn set_walk_speed(
        &mut self,
        player: Resource<Player>,
        speed: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_walk_speed(speed);
        Ok(())
    }

    async fn set_invulnerable(
        &mut self,
        player: Resource<Player>,
        invulnerable: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_invulnerable(invulnerable);
        Ok(())
    }

    async fn set_player_time(
        &mut self,
        player: Resource<Player>,
        time: u64,
        relative: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_player_time(time, relative);
        Ok(())
    }

    async fn reset_player_time(&mut self, player: Resource<Player>) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.reset_player_time();
        Ok(())
    }

    async fn get_player_time(&mut self, player: Resource<Player>) -> wasmtime::Result<Option<u64>> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_player_time())
    }

    async fn is_player_time_relative(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        Ok(player.is_player_time_relative())
    }

    async fn set_player_weather(
        &mut self,
        player: Resource<Player>,
        weather: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::PlayerWeather,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let w = match weather {
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::PlayerWeather::Clear => crate::entity::player::PlayerWeather::Clear,
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::PlayerWeather::Downfall => crate::entity::player::PlayerWeather::Downfall,
        };
        player.set_player_weather(w);
        Ok(())
    }

    async fn reset_player_weather(&mut self, player: Resource<Player>) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.reset_player_weather();
        Ok(())
    }

    async fn get_player_weather(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Option<crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::PlayerWeather>>{
        let player = player_from_resource(self, &player)?;
        Ok(player.get_player_weather().map(|w| match w {
            crate::entity::player::PlayerWeather::Clear => crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::PlayerWeather::Clear,
            crate::entity::player::PlayerWeather::Downfall => crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::PlayerWeather::Downfall,
        }))
    }

    async fn set_compass_target(
        &mut self,
        player: Resource<Player>,
        pos: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::Position,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let pos_vec = from_wasm_position(pos);
        let block_pos = papokin_util::math::position::BlockPos::new(
            pos_vec.x as i32,
            pos_vec.y as i32,
            pos_vec.z as i32,
        );
        player.set_compass_target(block_pos);
        Ok(())
    }

    async fn get_compass_target(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<
        crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::Position,
    > {
        let player = player_from_resource(self, &player)?;
        let target = player
            .get_compass_target()
            .unwrap_or(papokin_util::math::position::BlockPos::new(0, 0, 0));
        let vec3 = papokin_util::math::vector3::Vector3::new(
            f64::from(target.0.x),
            f64::from(target.0.y),
            f64::from(target.0.z),
        );
        Ok(to_wasm_position(vec3))
    }

    async fn set_respawn_location(
        &mut self,
        player: Resource<Player>,
        pos: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::Position,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let pos_vec = from_wasm_position(pos);
        let block_pos = papokin_util::math::position::BlockPos::new(
            pos_vec.x as i32,
            pos_vec.y as i32,
            pos_vec.z as i32,
        );
        player.set_respawn_location(block_pos);
        Ok(())
    }

    async fn get_respawn_location(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<
        Option<
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::Position,
        >,
    > {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_respawn_location().map(|p| {
            let vec3 = papokin_util::math::vector3::Vector3::new(
                f64::from(p.0.x),
                f64::from(p.0.y),
                f64::from(p.0.z),
            );
            to_wasm_position(vec3)
        }))
    }

    async fn hide_player(
        &mut self,
        player: Resource<Player>,
        other: Resource<Player>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let other = player_from_resource(self, &other)?;
        player.hide_player(other.gameprofile.id);
        Ok(())
    }

    async fn show_player(
        &mut self,
        player: Resource<Player>,
        other: Resource<Player>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let other = player_from_resource(self, &other)?;
        player.show_player(other.gameprofile.id);
        Ok(())
    }

    async fn can_see(
        &mut self,
        player: Resource<Player>,
        other: Resource<Player>,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        let other = player_from_resource(self, &other)?;
        Ok(player.can_see(&other.gameprofile.id))
    }

    async fn can_see_player(
        &mut self,
        player: Resource<Player>,
        other: Resource<Player>,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        let other = player_from_resource(self, &other)?;
        Ok(player.can_see(&other.gameprofile.id))
    }

    async fn set_tab_list_ping(
        &mut self,
        player: Resource<Player>,
        latency_ms: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_tab_list_ping(latency_ms);
        Ok(())
    }

    async fn set_item_cooldown(
        &mut self,
        player: Resource<Player>,
        item_id: String,
        ticks: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_item_cooldown(&item_id, ticks);
        Ok(())
    }

    async fn get_item_cooldown(
        &mut self,
        player: Resource<Player>,
        item_id: String,
    ) -> wasmtime::Result<Option<i32>> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_item_cooldown(&item_id))
    }

    async fn has_item_cooldown(
        &mut self,
        player: Resource<Player>,
        item_id: String,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        Ok(player.has_item_cooldown(&item_id))
    }

    async fn ray_trace_block(
        &mut self,
        player: Resource<Player>,
        max_distance: f64,
        include_fluids: bool,
    ) -> wasmtime::Result<Option<papokin::plugin::world::RayTraceBlockResult>> {
        let player = player_from_resource(self, &player)?;
        let start = player.living_entity.entity.get_eye_pos();
        let direction = player.living_entity.entity.get_looking_vector();
        let end = start + direction * max_distance;
        let world = player.living_entity.entity.world.load_full();

        let hit = world.ray_trace_block(start, end, include_fluids);

        Ok(hit.map(|(pos, face, hit_pos)| papokin::plugin::world::RayTraceBlockResult {
            pos: papokin::plugin::world::BlockPos {
                x: pos.0.x,
                y: pos.0.y,
                z: pos.0.z,
            },
            face: crate::plugin::loader::wasm::wasm_host::wit::v0_1::world::to_wasm_block_direction(face),
            hit_pos: to_wasm_position(hit_pos),
        }))
    }

    async fn ray_trace_entity(
        &mut self,
        player: Resource<Player>,
        max_distance: f64,
    ) -> wasmtime::Result<Option<papokin::plugin::world::RayTraceEntityResult>> {
        let player = player_from_resource(self, &player)?;
        let start = player.living_entity.entity.get_eye_pos();
        let direction = player.living_entity.entity.get_looking_vector();
        let end = start + direction * max_distance;
        let world = player.living_entity.entity.world.load_full();
        let self_id = player.living_entity.entity.entity_id;

        let hits = world.ray_trace_entities(start, end);
        for (hit_entity, hit_pos, distance) in hits {
            if hit_entity.get_entity().entity_id != self_id {
                let entity_res = self
                    .add_entity(hit_entity)
                    .map_err(|_| wasmtime::Error::msg("添加实体资源失败"))?;
                return Ok(Some(papokin::plugin::world::RayTraceEntityResult {
                    entity: entity_res,
                    hit_pos: to_wasm_position(hit_pos),
                    distance,
                }));
            }
        }

        Ok(None)
    }

    async fn get_target_entity(
        &mut self,
        player: Resource<Player>,
        max_distance: f64,
    ) -> wasmtime::Result<Option<Resource<papokin::plugin::world::Entity>>> {
        let res = self.ray_trace_entity(player, max_distance).await?;
        Ok(res.map(|r| r.entity))
    }

    async fn get_target_block(
        &mut self,
        player: Resource<Player>,
        max_distance: u32,
    ) -> wasmtime::Result<
        Option<
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::Position,
        >,
    > {
        let player = player_from_resource(self, &player)?;
        let res = player.get_target_block(
            &player.living_entity.entity.world.load_full(),
            f64::from(max_distance),
        );
        Ok(res.map(|p| {
            let vec3 = papokin_util::math::vector3::Vector3::new(
                f64::from(p.0.x),
                f64::from(p.0.y),
                f64::from(p.0.z),
            );
            to_wasm_position(vec3)
        }))
    }

    async fn get_target_block_exact(
        &mut self,
        player: Resource<Player>,
        max_distance: f64,
        include_fluids: bool,
    ) -> wasmtime::Result<Option<papokin::plugin::world::RayTraceBlockResult>> {
        self.ray_trace_block(player, max_distance, include_fluids)
            .await
    }

    async fn launch_projectile(
        &mut self,
        _player: Resource<Player>,
        _type_: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::player::ProjectileType,
    ) -> wasmtime::Result<
        Option<
            Resource<
                crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::world::Entity,
            >,
        >,
    > {
        Ok(None)
    }

    async fn set_tab_list_header_footer(
        &mut self,
        player: Resource<Player>,
        header: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
        footer: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<()> {
        let header = text_component_from_resource(self, &header);
        let footer = text_component_from_resource(self, &footer);
        let player = player_from_resource(self, &player)?;
        player.set_tab_list_header_footer(&header, &footer);
        Ok(())
    }

    async fn set_tab_list_order(
        &mut self,
        player: Resource<Player>,
        order: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_tab_list_order(order);
        Ok(())
    }

    async fn set_tab_list_latency(
        &mut self,
        player: Resource<Player>,
        latency: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_tab_list_latency(latency);
        Ok(())
    }

    async fn set_tab_list_listed(
        &mut self,
        player: Resource<Player>,
        listed: bool,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_tab_list_listed(listed);
        Ok(())
    }

    async fn show_title(
        &mut self,
        player: Resource<Player>,
        text: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<()> {
        let component = text_component_from_resource(self, &text);
        let player = player_from_resource(self, &player)?;
        player.show_title(&component, &TitleMode::Title);
        Ok(())
    }

    async fn show_subtitle(
        &mut self,
        player: Resource<Player>,
        text: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<()> {
        let component = text_component_from_resource(self, &text);
        let player = player_from_resource(self, &player)?;
        player.show_title(&component, &TitleMode::SubTitle);
        Ok(())
    }

    async fn show_actionbar(
        &mut self,
        player: Resource<Player>,
        text: wasmtime::component::Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<()> {
        let component = text_component_from_resource(self, &text);
        let player = player_from_resource(self, &player)?;
        player.show_title(&component, &TitleMode::ActionBar);
        Ok(())
    }

    async fn send_title_animation(
        &mut self,
        player: Resource<Player>,
        fade_in: i32,
        stay: i32,
        fade_out: i32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.send_title_animation(fade_in, stay, fade_out);
        Ok(())
    }

    async fn transfer(
        &mut self,
        player: Resource<Player>,
        host: String,
        port: u16,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player
            .client
            .send_packet(&papokin_protocol::java::client::play::CTransfer::new(
                &host,
                papokin_protocol::codec::var_int::VarInt(i32::from(port)),
            ))
            .await;
        Ok(())
    }

    async fn get_selected_slot(&mut self, player: Resource<Player>) -> wasmtime::Result<u8> {
        let player = player_from_resource(self, &player)?;
        Ok(player.inventory.get_selected_slot())
    }

    async fn get_health(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.living_entity.health.load())
    }

    async fn set_health(&mut self, player: Resource<Player>, health: f32) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_health(health);
        Ok(())
    }

    async fn get_max_health(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.living_entity.get_max_health())
    }

    async fn set_max_health(
        &mut self,
        player: Resource<Player>,
        max_health: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_max_health(max_health);
        Ok(())
    }

    async fn get_food_level(&mut self, player: Resource<Player>) -> wasmtime::Result<u8> {
        let player = player_from_resource(self, &player)?;
        Ok(player.hunger_manager.level.load())
    }

    async fn get_saturation(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.hunger_manager.saturation.load())
    }

    async fn set_saturation(
        &mut self,
        player: Resource<Player>,
        saturation: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_saturation(saturation);
        Ok(())
    }

    async fn get_exhaustion(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_exhaustion())
    }

    async fn set_exhaustion(
        &mut self,
        player: Resource<Player>,
        exhaustion: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_exhaustion(exhaustion);
        Ok(())
    }

    async fn get_absorption(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_absorption())
    }

    async fn set_absorption(
        &mut self,
        player: Resource<Player>,
        absorption: f32,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        player.set_absorption(absorption);
        Ok(())
    }

    async fn get_experience_level(&mut self, player: Resource<Player>) -> wasmtime::Result<i32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.experience_level.load(Ordering::Relaxed))
    }

    async fn get_experience_progress(&mut self, player: Resource<Player>) -> wasmtime::Result<f32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.experience_progress.load())
    }

    async fn get_experience_points(&mut self, player: Resource<Player>) -> wasmtime::Result<i32> {
        let player = player_from_resource(self, &player)?;
        Ok(player.experience_points.load(Ordering::Relaxed))
    }

    async fn is_flying(&mut self, player: Resource<Player>) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        Ok(player.is_flying())
    }

    async fn set_flying(&mut self, player: Resource<Player>, flying: bool) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        {
            let mut abilities = player
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            abilities.flying = flying;
        };
        player.send_abilities_update();
        Ok(())
    }

    async fn get_abilities(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<papokin::plugin::player::PlayerAbilities> {
        let player = player_from_resource(self, &player)?;
        let abilities = player
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(papokin::plugin::player::PlayerAbilities {
            invulnerable: abilities.invulnerable,
            flying: abilities.flying,
            allow_flying: abilities.allow_flying,
            creative: abilities.creative,
            allow_modify_world: abilities.allow_modify_world,
            fly_speed: abilities.fly_speed,
            walk_speed: abilities.walk_speed,
        })
    }

    async fn set_abilities(
        &mut self,
        player: Resource<Player>,
        abilities: papokin::plugin::player::PlayerAbilities,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        {
            let mut a = player
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            a.invulnerable = abilities.invulnerable;
            a.flying = abilities.flying;
            a.allow_flying = abilities.allow_flying;
            a.creative = abilities.creative;
            a.allow_modify_world = abilities.allow_modify_world;
            a.fly_speed = abilities.fly_speed;
            a.walk_speed = abilities.walk_speed;
        };
        player.send_abilities_update();
        Ok(())
    }

    async fn get_ip(&mut self, player: Resource<Player>) -> wasmtime::Result<String> {
        let player = player_from_resource(self, &player)?;
        Ok(player.get_ip())
    }

    async fn get_skin(&mut self, player: Resource<Player>) -> wasmtime::Result<Option<PlayerSkin>> {
        let player = player_from_resource(self, &player)?;
        Ok(player
            .gameprofile
            .properties
            .load()
            .iter()
            .find(|p| p.name.as_ref() == "textures")
            .map(|p| PlayerSkin {
                value: p.value.to_string(),
                signature: p.signature.as_ref().map(std::string::ToString::to_string),
            }))
    }

    async fn set_skin(
        &mut self,
        player: Resource<Player>,
        skin: PlayerSkin,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let mut properties = (**player.gameprofile.properties.load()).clone();

        properties.retain(|p| p.name.as_ref() != "textures");
        properties.push(Property {
            name: "textures".into(),
            value: skin.value.into(),
            signature: skin.signature.map(std::convert::Into::into),
        });

        player.gameprofile.properties.store(Arc::new(properties));

        Ok(())
    }

    async fn get_skin_parts(&mut self, player: Resource<Player>) -> wasmtime::Result<SkinParts> {
        let player = player_from_resource(self, &player)?;
        let mask = player.config.load().skin_parts;
        let mut parts = SkinParts::empty();
        if mask & 0x01 != 0 {
            parts |= SkinParts::CAPE;
        }
        if mask & 0x02 != 0 {
            parts |= SkinParts::JACKET;
        }
        if mask & 0x04 != 0 {
            parts |= SkinParts::LEFT_SLEEVE;
        }
        if mask & 0x08 != 0 {
            parts |= SkinParts::RIGHT_SLEEVE;
        }
        if mask & 0x10 != 0 {
            parts |= SkinParts::LEFT_PANTS_LEG;
        }
        if mask & 0x20 != 0 {
            parts |= SkinParts::RIGHT_PANTS_LEG;
        }
        if mask & 0x40 != 0 {
            parts |= SkinParts::HAT;
        }
        Ok(parts)
    }

    async fn set_skin_parts(
        &mut self,
        player: Resource<Player>,
        parts: SkinParts,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let mut mask = 0u8;
        if parts.contains(SkinParts::CAPE) {
            mask |= 0x01;
        }
        if parts.contains(SkinParts::JACKET) {
            mask |= 0x02;
        }
        if parts.contains(SkinParts::LEFT_SLEEVE) {
            mask |= 0x04;
        }
        if parts.contains(SkinParts::RIGHT_SLEEVE) {
            mask |= 0x08;
        }
        if parts.contains(SkinParts::LEFT_PANTS_LEG) {
            mask |= 0x10;
        }
        if parts.contains(SkinParts::RIGHT_PANTS_LEG) {
            mask |= 0x20;
        }
        if parts.contains(SkinParts::HAT) {
            mask |= 0x40;
        }

        {
            let mut config = (**player.config.load()).clone();
            config.skin_parts = mask;
            player.config.store(Arc::new(config));
        };
        player.send_client_information();
        Ok(())
    }

    async fn get_advancement_progress(
        &mut self,
        player: Resource<Player>,
        advancement_id: String,
    ) -> wasmtime::Result<Option<papokin::plugin::advancement::AdvancementProgress>> {
        let player = player_from_resource(self, &player)?;
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(
                &advancement_id,
            )
        else {
            return Ok(None);
        };
        let guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let progress = guard.progress.map.get(advancement).map_or_else(
            || papokin::plugin::advancement::AdvancementProgress {
                advancement_id: advancement.id.to_string(),
                done: false,
                awarded_criteria: Vec::new(),
                remaining_criteria: advancement
                    .criteria
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            },
            |progress| papokin::plugin::advancement::AdvancementProgress {
                advancement_id: advancement.id.to_string(),
                done: progress.is_done(),
                awarded_criteria: progress
                    .get_completed_criteria()
                    .map(|s| s.to_string())
                    .collect(),
                remaining_criteria: progress
                    .get_remaining_criteria()
                    .map(|s| s.to_string())
                    .collect(),
            },
        );
        Ok(Some(progress))
    }

    async fn revoke_advancement_criterion(
        &mut self,
        player: Resource<Player>,
        advancement_id: String,
        criterion: String,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(
                &advancement_id,
            )
        else {
            return Ok(false);
        };
        let mut guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let revoked = guard.revoke(advancement, &criterion);
        if revoked {
            guard.flush_dirty(&player, true);
        }
        Ok(revoked)
    }

    async fn revoke_advancement(
        &mut self,
        player: Resource<Player>,
        advancement_id: String,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(
                &advancement_id,
            )
        else {
            return Ok(false);
        };
        let mut guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let progress = guard.progress.get_mut_or_start_progress(advancement);
        if !progress.has_progress() {
            return Ok(false);
        }
        let completed: Vec<Arc<str>> = progress.get_completed_criteria().collect();
        let mut any_revoked = false;
        for criterion in completed {
            if guard.revoke(advancement, &criterion) {
                any_revoked = true;
            }
        }
        if any_revoked {
            guard.flush_dirty(&player, true);
        }
        Ok(any_revoked)
    }

    async fn has_advancement(
        &mut self,
        player: Resource<Player>,
        advancement_id: String,
    ) -> wasmtime::Result<bool> {
        let player = player_from_resource(self, &player)?;
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(
                &advancement_id,
            )
        else {
            return Ok(false);
        };
        let guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let done = guard
            .progress
            .map
            .get(advancement)
            .is_some_and(crate::entity::player::advancement::AdvancementProgress::is_done);
        Ok(done)
    }

    async fn get_completed_advancements(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Vec<String>> {
        let player = player_from_resource(self, &player)?;
        let guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let list = guard
            .progress
            .map
            .iter()
            .filter(|(_, p)| p.is_done())
            .map(|(adv, _)| adv.id.to_string())
            .collect();
        Ok(list)
    }

    async fn get_selected_advancement_tab(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Option<String>> {
        let player = player_from_resource(self, &player)?;
        let guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.last_selected_tab.map(|adv| adv.id.to_string()))
    }

    async fn set_selected_advancement_tab(
        &mut self,
        player: Resource<Player>,
        tab_id: Option<String>,
    ) -> wasmtime::Result<()> {
        let player = player_from_resource(self, &player)?;
        let target_adv = tab_id.as_deref().and_then(
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement,
        );
        let mut guard = player
            .advancements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.set_selected_tab(target_adv);
        Ok(())
    }

    async fn as_java(
        &mut self,
        player: Resource<Player>,
    ) -> wasmtime::Result<Option<Resource<papokin::plugin::player::JavaPlayer>>> {
        let player = player_from_resource(self, &player)?;
        Ok(Some(self.add_java_player(player)?))
    }

    async fn drop(&mut self, rep: Resource<Player>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<PlayerResource>(Resource::new_own(rep.rep()));
        Ok(())
    }
}

impl papokin::plugin::player::HostPlayerWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn open_ender_chest(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.open_ender_chest();
            })
            .await
    }

    async fn set_gamemode(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        mode: papokin::plugin::common::GameMode,
    ) -> wasmtime::Result<bool> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };
        let mode = from_wasm_game_mode(mode);

        plugin
            .store
            .pump_blocking(&mut host, move || player.set_gamemode(mode))
            .await
    }

    async fn set_permission_level(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        level: papokin::plugin::permission::PermissionLevel,
    ) -> wasmtime::Result<()> {
        let (player, server, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                state.server.as_ref().expect("服务器不可用").clone(),
                plugin_from_state(state)?,
            )
        };
        let level = from_wit_permission_level(level);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let command_dispatcher = server.command_dispatcher.load();
                player.set_permission_lvl(&server, level, &command_dispatcher);
            })
            .await
    }

    async fn has_permission(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        node: String,
    ) -> wasmtime::Result<bool> {
        let (player, server, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                state.server.as_ref().expect("服务器不可用").clone(),
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.has_permission(&server, &node))
            .await
    }

    async fn add_effect(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        effect: papokin::plugin::status_effect::StatusEffectInstance,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };
        let effect_type = super::status_effect::from_wasm_status_effect_type(effect.effect_type);
        let Some(status_effect) =
            papokin_data::effect::StatusEffect::from_name(effect_type.to_name())
        else {
            return Ok(());
        };
        let effect = papokin_data::potion::Effect {
            effect_type: status_effect,
            duration: effect.duration as i32,
            amplifier: effect.amplifier,
            ambient: effect.ambient,
            show_particles: effect.show_particles,
            show_icon: effect.show_icon,
            blend: false,
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.add_effect(effect))
            .await
    }

    async fn heal(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        amount: f32,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.heal(amount))
            .await
    }

    async fn damage(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        amount: f32,
        damage_type: WitDamageType,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };
        let damage_type = from_wit_damage_type(damage_type);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.damage(&*player, amount, damage_type);
            })
            .await
    }

    async fn damage_by_name(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        amount: f32,
        damage_type_name: String,
    ) -> wasmtime::Result<()> {
        let (player, plugin, damage_type) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
                super::living_entity::resolve_damage_type_by_name(state, &damage_type_name)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.damage_resolved(&*player, amount, &damage_type);
            })
            .await
    }

    async fn kill(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.kill())
            .await
    }

    async fn teleport(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        position: papokin::plugin::common::Position,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: Resource<World>,
    ) -> wasmtime::Result<()> {
        let (player, world, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                world_from_resource(state, &world),
                plugin_from_state(state)?,
            )
        };
        let position = from_wasm_position(position);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.teleport(position, yaw, pitch, world);
            })
            .await
    }

    async fn teleport_with_flags(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        position: papokin::plugin::common::Position,
        yaw: Option<f32>,
        pitch: Option<f32>,
        flags: TeleportFlags,
        world: Resource<World>,
    ) -> wasmtime::Result<()> {
        let (player, world, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                world_from_resource(state, &world),
                plugin_from_state(state)?,
            )
        };
        let position = from_wasm_position(position);
        let relatives = from_wit_teleport_flags(flags);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.teleport_with_relatives(position, yaw, pitch, &relatives, world);
            })
            .await
    }

    async fn teleport_world(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        world: Resource<World>,
        position: papokin::plugin::common::Position,
        yaw: Option<f32>,
        pitch: Option<f32>,
    ) -> wasmtime::Result<()> {
        let (player, world, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                world_from_resource(state, &world),
                plugin_from_state(state)?,
            )
        };
        let position = from_wasm_position(position);
        let runtime = tokio::runtime::Handle::current();

        plugin
            .store
            .pump_blocking(&mut host, move || {
                runtime.block_on(player.teleport_world(world, position, yaw, pitch));
            })
            .await
    }

    async fn respawn(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };
        let runtime = tokio::runtime::Handle::current();

        plugin
            .store
            .pump_blocking(&mut host, move || runtime.block_on(player.respawn()))
            .await
    }

    async fn open_gui(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        gui: Resource<papokin::plugin::gui::Gui>,
    ) -> wasmtime::Result<()> {
        let (player, gui, plugin) = {
            let state = host.get();
            let gui = state
                .resource_table
                .get::<GuiResource>(&Resource::new_own(gui.rep()))
                .map_err(|_| wasmtime::Error::msg("无效的 GUI 资源句柄"))?
                .provider
                .clone();
            (
                player_from_resource(state, &player)?,
                gui,
                plugin_from_state(state)?,
            )
        };

        let (window_type, inventory, allow_grab_items, allow_put_items, title) = plugin
            .store
            .pump_reentry(&mut host, async move {
                let gui = gui.lock().await;
                (
                    gui.window_type,
                    gui.inventory.clone(),
                    gui.allow_grab_items,
                    gui.allow_put_items,
                    gui.title.clone(),
                )
            })
            .await?;

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.increment_screen_handler_sync_id();
                let sync_id = player.screen_handler_sync_id.load(Ordering::Relaxed);
                let screen_handler: Arc<
                    std::sync::Mutex<dyn papokin_inventory::screen_handler::ScreenHandler>,
                > = Arc::new(std::sync::Mutex::new(PluginScreenHandler::new(
                    sync_id,
                    window_type,
                    &inventory,
                    allow_grab_items,
                    allow_put_items,
                )));
                player.open_handled_screen_direct(screen_handler, &title);
            })
            .await
    }

    async fn ban(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        options: papokin::plugin::player::BanPlayerOptions,
    ) -> wasmtime::Result<()> {
        let papokin::plugin::player::BanPlayerOptions {
            reason,
            source,
            expires_at_utc,
            duration_seconds,
            kick_if_online,
            log_to_console,
        } = options;
        let (player, server, reason, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                state.server.as_ref().expect("服务器不可用").clone(),
                reason
                    .as_ref()
                    .map(|reason| text_component_from_resource(state, reason)),
                plugin_from_state(state)?,
            )
        };
        let expires = parse_ban_expiry(expires_at_utc, duration_seconds);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.ban_explicit(
                    &server,
                    reason,
                    source,
                    expires,
                    kick_if_online,
                    log_to_console,
                );
            })
            .await
    }

    async fn ban_ip(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        options: papokin::plugin::player::BanIpOptions,
    ) -> wasmtime::Result<()> {
        let papokin::plugin::player::BanIpOptions {
            reason,
            source,
            expires_at_utc,
            duration_seconds,
            kick_matching_players,
            log_to_console,
        } = options;
        let (player, server, reason, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                state.server.as_ref().expect("服务器不可用").clone(),
                reason
                    .as_ref()
                    .map(|reason| text_component_from_resource(state, reason)),
                plugin_from_state(state)?,
            )
        };
        let expires = parse_ban_expiry(expires_at_utc, duration_seconds);

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.ban_ip_explicit(
                    &server,
                    reason,
                    source,
                    expires,
                    kick_matching_players,
                    log_to_console,
                );
            })
            .await
    }

    async fn set_food_level(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        level: u8,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.set_food_level(level))
            .await
    }

    async fn set_experience_level(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        level: i32,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.set_experience_level(level, true))
            .await
    }

    async fn set_experience_progress(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        progress: f32,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.set_experience(
                    player.experience_level.load(Ordering::Relaxed),
                    progress,
                    player.experience_points.load(Ordering::Relaxed),
                );
            })
            .await
    }

    async fn set_experience_points(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        points: i32,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                player.set_experience(
                    player.experience_level.load(Ordering::Relaxed),
                    player.experience_progress.load(),
                    points,
                );
            })
            .await
    }

    async fn add_experience_levels(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        levels: i32,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.add_experience_levels(levels))
            .await
    }

    async fn add_experience_points(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        points: i32,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };

        plugin
            .store
            .pump_blocking(&mut host, move || player.add_experience_points(points))
            .await
    }

    async fn award_advancement_criterion(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        advancement_id: String,
        criterion: String,
    ) -> wasmtime::Result<bool> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(
                &advancement_id,
            )
        else {
            return Ok(false);
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let result = {
                    let mut guard = player
                        .advancements
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    guard.award(advancement, &criterion)
                };

                PlayerAdvancement::finish_award(&player, advancement, result);
                if result.awarded() {
                    let mut guard = player
                        .advancements
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    guard.flush_dirty(&player, true);
                }
                result.awarded()
            })
            .await
    }

    async fn award_advancement(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<Player>,
        advancement_id: String,
    ) -> wasmtime::Result<bool> {
        let (player, plugin) = {
            let state = host.get();
            (
                player_from_resource(state, &player)?,
                plugin_from_state(state)?,
            )
        };
        let Some(advancement) =
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::advancement::find_advancement(
                &advancement_id,
            )
        else {
            return Ok(false);
        };

        plugin
            .store
            .pump_blocking(&mut host, move || {
                let result = {
                    let mut guard = player
                        .advancements
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let progress = guard.progress.get_mut_or_start_progress(advancement);
                    if progress.is_done() {
                        return false;
                    }
                    let remaining: Vec<Arc<str>> = progress.get_remaining_criteria().collect();
                    let mut result = AdvancementAward::default();
                    for criterion in remaining {
                        result = result.combine(guard.award(advancement, &criterion));
                    }
                    result
                };

                PlayerAdvancement::finish_award(&player, advancement, result);
                if result.awarded() {
                    let mut guard = player
                        .advancements
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    guard.flush_dirty(&player, true);
                }
                result.awarded()
            })
            .await
    }
}

impl papokin::plugin::player::HostJavaPlayer for PluginHostState {
    async fn get_version(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<papokin::plugin::player::JavaMinecraftVersion> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let client = &player.client;
        Ok(to_wasm_java_version(client.version.load()))
    }

    async fn get_brand(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<String> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let client = &player.client;
        Ok(client.brand.load().as_ref().clone().unwrap_or_default())
    }

    async fn get_server_address(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<String> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let client = &player.client;
        Ok(client.server_address.clone())
    }

    async fn get_settings(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<papokin::plugin::player::JavaPlayerSettings> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let config = player.config.load();
        let mask = config.skin_parts;
        let mut parts = SkinParts::empty();
        if mask & 0x01 != 0 {
            parts |= SkinParts::CAPE;
        }
        if mask & 0x02 != 0 {
            parts |= SkinParts::JACKET;
        }
        if mask & 0x04 != 0 {
            parts |= SkinParts::LEFT_SLEEVE;
        }
        if mask & 0x08 != 0 {
            parts |= SkinParts::RIGHT_SLEEVE;
        }
        if mask & 0x10 != 0 {
            parts |= SkinParts::LEFT_PANTS_LEG;
        }
        if mask & 0x20 != 0 {
            parts |= SkinParts::RIGHT_PANTS_LEG;
        }
        if mask & 0x40 != 0 {
            parts |= SkinParts::HAT;
        }

        Ok(papokin::plugin::player::JavaPlayerSettings {
            locale: config.locale.clone(),
            view_distance: config.view_distance.get(),
            chat_mode: to_wasm_chat_mode(&config.chat_mode),
            chat_colors: config.chat_colors,
            skin_parts: parts,
            main_hand: match config.main_hand {
                papokin_util::Hand::Left => papokin::plugin::common::Hand::Left,
                papokin_util::Hand::Right => papokin::plugin::common::Hand::Right,
            },
            text_filtering: config.text_filtering,
            server_listing: config.server_listing,
        })
    }

    async fn send_packet(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
        packet: papokin::plugin::java_packets::ClientboundPacket,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let client = &player.client;
        // 由访客控制的数据包字段可能导致生成的
        // `try_into().unwrap()` 转换；绝不能让该 panic 跨越
        // 宿主调用边界——此时应直接丢弃数据包。
        let version = client.version.load();
        let serialized = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::generated_packets::serialize_java_packet(
                &packet, version,
            )
        }));
        match serialized {
            Ok(Some(bytes)) => client.send_packet_now_data(bytes).await,
            Ok(None) => {}
            Err(_) => {
                tracing::error!("插件提供的 Java 数据包在序列化时 panic；已将其丢弃");
            }
        }
        Ok(())
    }

    async fn send_custom_payload(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
        channel: String,
        data: Vec<u8>,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        player
            .send_client_packet(&papokin_protocol::java::client::play::CCustomPayload::new(
                &channel, &data,
            ))
            .await;
        Ok(())
    }

    async fn get_scoreboard(
        &mut self,
        player_res: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<Resource<papokin::plugin::scoreboard::Scoreboard>> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player_res.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();
        self.add_scoreboard(
            crate::plugin::loader::wasm::wasm_host::state::ScoreboardProvider::Player(player),
        )
    }

    async fn reset_scoreboard(
        &mut self,
        player_res: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player_res.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();
        player.reset_scoreboard();
        Ok(())
    }

    async fn send_resource_pack(
        &mut self,
        player_res: Resource<papokin::plugin::player::JavaPlayer>,
        pack: papokin::plugin::player::JavaResourcePack,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player_res.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let uuid = Uuid::from_wit(&pack.id);
        let prompt_message = pack
            .prompt_message
            .as_ref()
            .map(|p| text_component_from_resource(self, p));

        player
            .client
            .send_packet(
                &papokin_protocol::java::client::play::CAddResourcePack::new(
                    &uuid,
                    &pack.url,
                    &pack.hash,
                    pack.forced,
                    prompt_message,
                ),
            )
            .await;
        Ok(())
    }

    async fn remove_resource_pack(
        &mut self,
        player_res: Resource<papokin::plugin::player::JavaPlayer>,
        id: papokin::plugin::uuid::Uuid,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player_res.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let uuid = Uuid::from_wit(&id);

        player
            .client
            .send_packet(
                &papokin_protocol::java::client::play::CRemoveResourcePack::new(Some(&uuid)),
            )
            .await;
        Ok(())
    }

    async fn clear_resource_packs(
        &mut self,
        player_res: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player_res.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        player
            .client
            .send_packet(&papokin_protocol::java::client::play::CRemoveResourcePack::new(None))
            .await;
        Ok(())
    }

    async fn send_game_event(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
        event: papokin::plugin::player::ClientGameEvent,
        value: f32,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        let internal_event = from_wit_client_game_event(event);
        player.send_game_event(internal_event, value);
        Ok(())
    }

    async fn send_entity_status(
        &mut self,
        player: Resource<papokin::plugin::player::JavaPlayer>,
        entity_id: i32,
        status: papokin::plugin::entity_statuses::EntityStatus,
    ) -> wasmtime::Result<()> {
        let player = self
            .resource_table
            .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                &Resource::new_own(player.rep()),
            )
            .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
            .provider
            .clone();

        // SAFETY: WIT 枚举的变体与判别值由 entity_statuses.json 1:1 生成
        let internal_status: papokin_data::entity_status::EntityStatus =
            unsafe { std::mem::transmute(status as u8) };
        let packet = papokin_protocol::java::client::play::CEntityStatus::new(
            entity_id,
            internal_status as u8 as i8,
        );
        player.try_send_client_packet(&packet);
        Ok(())
    }

    async fn drop(
        &mut self,
        rep: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
            Resource::new_own(rep.rep()),
        );
        Ok(())
    }
}

impl papokin::plugin::player::HostJavaPlayerWithStore<PluginHostState>
    for HasSelf<PluginHostState>
{
    #[allow(clippy::too_many_lines)]
    async fn show_dialog(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<papokin::plugin::player::JavaPlayer>,
        dialog: Dialog,
    ) -> wasmtime::Result<()> {
        let (player, protocol_dialog, plugin) = {
            let state = host.get();
            let player = state
                .resource_table
                .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                    &Resource::new_own(player.rep()),
                )
                .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
                .provider
                .clone();
            let protocol_dialog = super::events::dialog::protocol_dialog_from_wasm(state, &dialog);
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (player, protocol_dialog, plugin)
        };

        let operation = async move {
            if let Some(server) = player.world().server.upgrade() {
                let mut event =
                    crate::plugin::api::events::dialog::dialog_show::DialogShowEvent::new(
                        player.clone(),
                        protocol_dialog.clone(),
                    );
                server.plugin_manager.fire(&server, &mut event).await;
                if event.cancelled {
                    return Ok(());
                }
            }

            let client = &player.client;
            {
                match client.connection_state.load() {
                    papokin_protocol::ConnectionState::Config => {
                        client
                            .send_packet(
                                &papokin_protocol::java::client::config::CConfigShowDialog::new(
                                    papokin_protocol::IdOr::Value(DialogNBT::from_dialog(
                                        &protocol_dialog,
                                    )),
                                ),
                            )
                            .await;
                    }
                    papokin_protocol::ConnectionState::Play => {
                        client
                            .send_packet(
                                &papokin_protocol::java::client::play::CPlayShowDialog::new(
                                    papokin_protocol::IdOr::Value(DialogNBT::from_dialog(
                                        &protocol_dialog,
                                    )),
                                ),
                            )
                            .await;
                    }
                    _ => {}
                }
            }

            Ok(())
        };

        plugin.store.pump_reentry(&mut host, operation).await?
    }

    async fn clear_dialog(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<papokin::plugin::player::JavaPlayer>,
    ) -> wasmtime::Result<()> {
        let (player, plugin) = {
            let state = host.get();
            let player = state
                .resource_table
                .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                    &Resource::new_own(player.rep()),
                )
                .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
                .provider
                .clone();
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (player, plugin)
        };

        let operation = async move {
            if let Some(server) = player.world().server.upgrade() {
                let mut event =
                    crate::plugin::api::events::dialog::dialog_clear::DialogClearEvent::new(
                        player.clone(),
                    );
                server.plugin_manager.fire(&server, &mut event).await;
                if event.cancelled {
                    return Ok(());
                }
            }

            let client = &player.client;
            {
                match client.connection_state.load() {
                    papokin_protocol::ConnectionState::Config => {
                        client
                            .send_packet(
                                &papokin_protocol::java::client::config::CConfigClearDialog::new(),
                            )
                            .await;
                    }
                    papokin_protocol::ConnectionState::Play => {
                        client
                            .send_packet(
                                &papokin_protocol::java::client::play::CPlayClearDialog::new(),
                            )
                            .await;
                    }
                    _ => {}
                }
            }

            Ok(())
        };

        plugin.store.pump_reentry(&mut host, operation).await?
    }

    async fn kick(
        mut host: Access<'_, PluginHostState, Self>,
        player: Resource<papokin::plugin::player::JavaPlayer>,
        options: papokin::plugin::player::JavaKickOptions,
    ) -> wasmtime::Result<()> {
        let (player, reason, plugin) = {
            let state = host.get();
            let player = state
                .resource_table
                .get::<crate::plugin::loader::wasm::wasm_host::state::JavaPlayerResource>(
                    &Resource::new_own(player.rep()),
                )
                .map_err(|_| wasmtime::Error::msg("无效的 java-player 资源句柄"))?
                .provider
                .clone();
            let reason = text_component_from_resource(state, &options.reason);
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (player, reason, plugin)
        };

        if options.log_to_console {
            tracing::info!(
                "正在踢出 Java 玩家 {}（{}）：{}",
                player.gameprofile.name,
                player.gameprofile.id,
                reason.clone().to_pretty_console()
            );
        }

        let operation = async move {
            if let Some(server) = player.world().server.upgrade()
                && let Some(player_arc) = player.world().get_player_by_uuid(player.gameprofile.id)
            {
                let mut event =
                    crate::plugin::api::events::player::player_kick::PlayerKickEvent::new(
                        player_arc,
                        reason.clone().to_pretty_console(),
                    );
                server.plugin_manager.fire(&server, &mut event).await;
                if event.cancelled {
                    return Ok(());
                }
            }

            let send_packet = options.teardown_policy
                != papokin::plugin::player::SocketTeardownPolicy::DropConnection;
            player.client.kick_explicit(&reason, send_packet).await;

            Ok(())
        };

        plugin.store.pump_reentry(&mut host, operation).await?
    }
}
