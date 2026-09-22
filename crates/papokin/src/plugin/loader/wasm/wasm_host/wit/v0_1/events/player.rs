use std::sync::Arc;
use tokio::sync::Mutex;

use crate::plugin::{
    loader::wasm::wasm_host::{
        state::PluginHostState,
        wit::v0_1::{
            events::{
                ToFromWasmEvent, cleanup_event, consume_player, consume_text_component,
                consume_world, from_wasm_block_name, from_wasm_block_position,
                from_wasm_click_type, from_wasm_entity_interaction_action, from_wasm_entity_type,
                from_wasm_game_mode, from_wasm_hand, from_wasm_position, to_wasm_block_position,
                to_wasm_click_type, to_wasm_entity_interaction_action, to_wasm_entity_type,
                to_wasm_game_mode, to_wasm_hand, to_wasm_position,
            },
            gui::{from_wit_screen, to_wit_screen},
            papokin::plugin::event::{
                AsyncPlayerChatEventData, AsyncPlayerPreLoginEventData,
                AsyncPlayerSendSuggestionsEventData, AsyncTabCompleteEventData,
                ClientTickEndEventData, Event, FillProfileEventData, Gs4QueryEventData,
                HandshakeIntention as WasmHandshakeIntention, InteractAction as WasmInteractAction,
                InventoryClickEventData, InventoryCloseEventData,
                ItemFrameAction as WasmItemFrameAction, LookupProfileEventData,
                PlayerAdvancementDoneEventData, PlayerAnimationEventData,
                PlayerArmorChangeEventData, PlayerArmorStandManipulateEventData,
                PlayerAttackEntityCooldownResetEventData, PlayerBedEnterEventData,
                PlayerBedFailEnterEventData, PlayerBedLeaveEventData, PlayerBucketEmptyEventData,
                PlayerBucketEntityEventData, PlayerBucketFillEventData, PlayerChangeWorldEventData,
                PlayerChangedMainHandEventData, PlayerChangedWorldEventData,
                PlayerChannelEventData, PlayerChatEventData, PlayerChunkUnloadEventData,
                PlayerClientOptionsChangeEventData, PlayerCommandPreprocessEventData,
                PlayerCommandSendEventData, PlayerConnectionCloseEventData,
                PlayerCustomPayloadEventData, PlayerDeepSleepEventData, PlayerDropItemEventData,
                PlayerEditBookEventData, PlayerEggThrowEventData, PlayerElytraBoostEventData,
                PlayerExpChangeEventData, PlayerExpCooldownChangeEventData, PlayerFishEventData,
                PlayerFishState as WasmPlayerFishState, PlayerFlowerPotManipulateEventData,
                PlayerGamemodeChangeEventData, PlayerHandshakeEventData,
                PlayerHarvestBlockEventData, PlayerHideEntityEventData, PlayerInputEventData,
                PlayerInsertLecternBookEventData, PlayerInteractAtEntityEventData,
                PlayerInteractEntityEventData, PlayerInteractEventData,
                PlayerInteractUnknownEntityEventData, PlayerInventorySlotChangeEventData,
                PlayerItemBreakEventData, PlayerItemConsumeEventData, PlayerItemCooldownEventData,
                PlayerItemDamageEventData, PlayerItemFrameChangeEventData,
                PlayerItemGroupCooldownEventData, PlayerItemHeldEventData, PlayerItemMendEventData,
                PlayerJoinEventData, PlayerJumpEventData, PlayerKickEventData,
                PlayerLeashEntityEventData, PlayerLeaveEventData, PlayerLecternPageChangeEventData,
                PlayerLevelChangeEventData, PlayerLinksSendEventData, PlayerLocaleChangeEventData,
                PlayerLoginEventData, PlayerLoomPatternSelectEventData, PlayerMoveEventData,
                PlayerNameEntityEventData, PlayerNaturallySpawnCreaturesEventData,
                PlayerOpenSignEventData, PlayerPermissionCheckEventData, PlayerPickBlockEventData,
                PlayerPickEntityEventData, PlayerPickupArrowEventData,
                PlayerPickupExperienceEventData, PlayerPortalEventData, PlayerPostRespawnEventData,
                PlayerPreLoginEventData, PlayerPurchaseEventData, PlayerReadyArrowEventData,
                PlayerRecipeBookClickEventData, PlayerRecipeBookSettingsChangeEventData,
                PlayerRecipeDiscoverEventData, PlayerRegisterChannelEventData,
                PlayerResourcePackStatusEventData, PlayerRespawnEventData, PlayerRiptideEventData,
                PlayerServerFullCheckEventData, PlayerShearEntityEventData,
                PlayerShowEntityEventData, PlayerSignCommandPreprocessEventData,
                PlayerSpawnChangeEventData, PlayerSpawnLocationEventData,
                PlayerStartSpectatingEntityEventData, PlayerStatisticIncrementEventData,
                PlayerStonecutterRecipeSelectEventData, PlayerStopSpectatingEntityEventData,
                PlayerStopUsingItemEventData, PlayerSwapHandsEventData,
                PlayerSwapWithEquipmentSlotEventData, PlayerTakeLecternBookEventData,
                PlayerTeleportEndGatewayEventData, PlayerTeleportEventData,
                PlayerToggleFlightEventData, PlayerToggleSneakEventData,
                PlayerToggleSprintEventData, PlayerTrackEntityEventData, PlayerTradeEventData,
                PlayerUnleashEntityEventData, PlayerUnregisterChannelEventData,
                PlayerUntrackEntityEventData, PlayerVelocityEventData, PreFillProfileEventData,
                PreLookupProfileEventData, PrePlayerAttackEntityEventData,
                ServerFullCheckResult as WasmServerFullCheckResult, UncheckedSignChangeEventData,
            },
            papokin::plugin::uuid::Uuid as WitUuid,
            uuid::UuidExt,
        },
    },
    player::{
        async_player_chat::AsyncPlayerChatEvent,
        async_player_pre_login::AsyncPlayerPreLoginEvent,
        async_player_send_suggestions::AsyncPlayerSendSuggestionsEvent,
        async_tab_complete::AsyncTabCompleteEvent,
        changed_main_hand::PlayerChangedMainHandEvent,
        client_tick_end::ClientTickEndEvent,
        egg_throw::PlayerEggThrowEvent,
        exp_change::PlayerExpChangeEvent,
        fill_profile::FillProfileEvent,
        fish::{PlayerFishEvent, PlayerFishState},
        gs4_query::Gs4QueryEvent,
        inventory_close::InventoryCloseEvent,
        inventory_interact::InventoryClickEvent,
        item_held::PlayerItemHeldEvent,
        lookup_profile::LookupProfileEvent,
        player_advancement_done::PlayerAdvancementDoneEvent,
        player_animation::{PlayerAnimationEvent, PlayerAnimationType},
        player_armor_change::PlayerArmorChangeEvent,
        player_armor_stand_manipulate::PlayerArmorStandManipulateEvent,
        player_attack_entity_cooldown_reset::PlayerAttackEntityCooldownResetEvent,
        player_bed_fail_enter::PlayerBedFailEnterEvent,
        player_bucket_entity::PlayerBucketEntityEvent,
        player_change_world::PlayerChangeWorldEvent,
        player_changed_world::PlayerChangedWorldEvent,
        player_channel::PlayerChannelEvent,
        player_chat::PlayerChatEvent,
        player_chunk_unload::PlayerChunkUnloadEvent,
        player_client_options_change::PlayerClientOptionsChangeEvent,
        player_command_preprocess::PlayerCommandPreprocessEvent,
        player_command_send::PlayerCommandSendEvent,
        player_connection_close::PlayerConnectionCloseEvent,
        player_custom_payload::PlayerCustomPayloadEvent,
        player_deep_sleep::PlayerDeepSleepEvent,
        player_edit_book::PlayerEditBookEvent,
        player_elytra_boost::PlayerElytraBoostEvent,
        player_exp_cooldown_change::PlayerExpCooldownChangeEvent,
        player_flower_pot_manipulate::PlayerFlowerPotManipulateEvent,
        player_gamemode_change::PlayerGamemodeChangeEvent,
        player_handshake::{HandshakeIntention, PlayerHandshakeEvent},
        player_harvest_block::PlayerHarvestBlockEvent,
        player_hide_entity::PlayerHideEntityEvent,
        player_input::PlayerInputEvent,
        player_insert_lectern_book::PlayerInsertLecternBookEvent,
        player_interact_at_entity::PlayerInteractAtEntityEvent,
        player_interact_entity_event::PlayerInteractEntityEvent,
        player_interact_event::{InteractAction, PlayerInteractEvent},
        player_interact_unknown_entity_event::PlayerInteractUnknownEntityEvent,
        player_inventory_slot_change::PlayerInventorySlotChangeEvent,
        player_item_break::PlayerItemBreakEvent,
        player_item_cooldown::PlayerItemCooldownEvent,
        player_item_frame_change::{ItemFrameAction, PlayerItemFrameChangeEvent},
        player_item_group_cooldown::PlayerItemGroupCooldownEvent,
        player_item_mend::PlayerItemMendEvent,
        player_join::PlayerJoinEvent,
        player_jump::PlayerJumpEvent,
        player_kick::PlayerKickEvent,
        player_leash_entity::PlayerLeashEntityEvent,
        player_leave::PlayerLeaveEvent,
        player_lectern_page_change::PlayerLecternPageChangeEvent,
        player_level_change::PlayerLevelChangeEvent,
        player_links_send::PlayerLinksSendEvent,
        player_locale_change::PlayerLocaleChangeEvent,
        player_login::PlayerLoginEvent,
        player_loom_pattern_select::PlayerLoomPatternSelectEvent,
        player_move::PlayerMoveEvent,
        player_name_entity::PlayerNameEntityEvent,
        player_naturally_spawn_creatures::PlayerNaturallySpawnCreaturesEvent,
        player_open_sign::PlayerOpenSignEvent,
        player_permission_check::PlayerPermissionCheckEvent,
        player_pick_block::PlayerPickBlockEvent,
        player_pick_entity::PlayerPickEntityEvent,
        player_pickup_arrow::PlayerPickupArrowEvent,
        player_pickup_experience::PlayerPickupExperienceEvent,
        player_portal::PlayerPortalEvent,
        player_post_respawn::PlayerPostRespawnEvent,
        player_pre_login::PlayerPreLoginEvent,
        player_purchase::PlayerPurchaseEvent,
        player_ready_arrow::PlayerReadyArrowEvent,
        player_recipe_book_click::PlayerRecipeBookClickEvent,
        player_recipe_book_settings_change::PlayerRecipeBookSettingsChangeEvent,
        player_recipe_discover::PlayerRecipeDiscoverEvent,
        player_register_channel::PlayerRegisterChannelEvent,
        player_resource_pack_status::PlayerResourcePackStatusEvent,
        player_respawn::PlayerRespawnEvent,
        player_riptide::PlayerRiptideEvent,
        player_server_full_check::{PlayerServerFullCheckEvent, ServerFullCheckResult},
        player_shear_entity::PlayerShearEntityEvent,
        player_show_entity::PlayerShowEntityEvent,
        player_sign_command_preprocess::PlayerSignCommandPreprocessEvent,
        player_spawn_change::PlayerSpawnChangeEvent,
        player_spawn_location::PlayerSpawnLocationEvent,
        player_start_spectating_entity::PlayerStartSpectatingEntityEvent,
        player_statistic_increment::PlayerStatisticIncrementEvent,
        player_stonecutter_recipe_select::PlayerStonecutterRecipeSelectEvent,
        player_stop_spectating_entity::PlayerStopSpectatingEntityEvent,
        player_stop_using_item::PlayerStopUsingItemEvent,
        player_swap_hands::PlayerSwapHandItemsEvent,
        player_swap_with_equipment_slot::PlayerSwapWithEquipmentSlotEvent,
        player_take_lectern_book::PlayerTakeLecternBookEvent,
        player_teleport::PlayerTeleportEvent,
        player_teleport_end_gateway::PlayerTeleportEndGatewayEvent,
        player_toggle_flight_event::PlayerToggleFlightEvent,
        player_toggle_sneak_event::PlayerToggleSneakEvent,
        player_toggle_sprint_event::PlayerToggleSprintEvent,
        player_track_entity::PlayerTrackEntityEvent,
        player_trade::PlayerTradeEvent,
        player_unleash_entity::PlayerUnleashEntityEvent,
        player_unregister_channel::PlayerUnregisterChannelEvent,
        player_untrack_entity::PlayerUntrackEntityEvent,
        player_velocity::PlayerVelocityEvent,
        pre_fill_profile::PreFillProfileEvent,
        pre_lookup_profile::PreLookupProfileEvent,
        pre_player_attack_entity::PrePlayerAttackEntityEvent,
        unchecked_sign_change::UncheckedSignChangeEvent,
    },
};
use bytes::Bytes;

const fn to_wasm_fish_state(state: PlayerFishState) -> WasmPlayerFishState {
    match state {
        PlayerFishState::Fishing => WasmPlayerFishState::Fishing,
        PlayerFishState::CaughtFish => WasmPlayerFishState::CaughtFish,
        PlayerFishState::CaughtEntity => WasmPlayerFishState::CaughtEntity,
        PlayerFishState::InGround => WasmPlayerFishState::InGround,
        PlayerFishState::FailedAttempt => WasmPlayerFishState::FailedAttempt,
        PlayerFishState::ReelIn => WasmPlayerFishState::ReelIn,
        PlayerFishState::Bite => WasmPlayerFishState::Bite,
    }
}

const fn from_wasm_fish_state(state: WasmPlayerFishState) -> PlayerFishState {
    match state {
        WasmPlayerFishState::Fishing => PlayerFishState::Fishing,
        WasmPlayerFishState::CaughtFish => PlayerFishState::CaughtFish,
        WasmPlayerFishState::CaughtEntity => PlayerFishState::CaughtEntity,
        WasmPlayerFishState::InGround => PlayerFishState::InGround,
        WasmPlayerFishState::FailedAttempt => PlayerFishState::FailedAttempt,
        WasmPlayerFishState::ReelIn => PlayerFishState::ReelIn,
        WasmPlayerFishState::Bite => PlayerFishState::Bite,
    }
}

impl ToFromWasmEvent for InventoryCloseEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::InventoryCloseEvent(InventoryCloseEventData {
            player,
            window_type: self.window_type.map(to_wit_screen),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::InventoryCloseEvent(data) => Self {
                player: consume_player(state, &data.player),
                window_type: data.window_type.map(from_wit_screen),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for InventoryClickEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::InventoryClickEvent(InventoryClickEventData {
            player,
            window_type: self.window_type.map(to_wit_screen),
            click_type: to_wasm_click_type(self.click_type),
            slot: self.slot,
            raw_slot: self.raw_slot,
            clicked_item: self.clicked_item.as_ref().map(|stack| {
                state
                    .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                    .expect("添加物品堆资源失败")
            }),
            cursor: self.cursor.as_ref().map(|stack| {
                state
                    .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                    .expect("添加物品堆资源失败")
            }),
            hotbar_button: self.hotbar_button,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::InventoryClickEvent(data) => Self {
                player: consume_player(state, &data.player),
                window_type: data.window_type.map(from_wit_screen),
                click_type: from_wasm_click_type(data.click_type),
                slot: data.slot,
                raw_slot: data.raw_slot,
                clicked_item: None, // 我们不会从 WASM 侧修改 clicked_item
                cursor: None,       // 我们不会从 WASM 侧修改光标
                hotbar_button: data.hotbar_button,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerJoinEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let join_message = state
            .add_text_component(self.join_message.clone())
            .expect("添加文本组件资源失败");

        Event::PlayerJoinEvent(PlayerJoinEventData {
            player,
            join_message,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerJoinEvent(data) => Self {
                player: consume_player(state, &data.player),
                join_message: consume_text_component(state, &data.join_message),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLeaveEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let leave_message = state
            .add_text_component(self.leave_message.clone())
            .expect("添加文本组件资源失败");

        Event::PlayerLeaveEvent(PlayerLeaveEventData {
            player,
            leave_message,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLeaveEvent(data) => Self {
                player: consume_player(state, &data.player),
                leave_message: consume_text_component(state, &data.leave_message),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLoginEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let kick_message = state
            .add_text_component(self.kick_message.clone())
            .expect("添加文本组件资源失败");

        Event::PlayerLoginEvent(PlayerLoginEventData {
            player,
            kick_message,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLoginEvent(data) => Self {
                player: consume_player(state, &data.player),
                kick_message: consume_text_component(state, &data.kick_message),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerChatEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let recipients = self
            .recipients
            .iter()
            .cloned()
            .map(|recipient| state.add_player(recipient).expect("添加玩家资源失败"))
            .collect();

        Event::PlayerChatEvent(PlayerChatEventData {
            player,
            message: self.message.clone(),
            recipients,
            signature: self.signature.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerChatEvent(data) => Self {
                player: consume_player(state, &data.player),
                message: data.message,
                recipients: data
                    .recipients
                    .into_iter()
                    .map(|recipient| consume_player(state, &recipient))
                    .collect(),
                signature: data.signature,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerCommandSendEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerCommandSendEvent(PlayerCommandSendEventData {
            player,
            command: self.command.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerCommandSendEvent(data) => Self {
                player: consume_player(state, &data.player),
                command: data.command,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPermissionCheckEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerPermissionCheckEvent(PlayerPermissionCheckEventData {
            player,
            permission: self.permission.clone(),
            permission_result: self.result,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPermissionCheckEvent(data) => Self {
                player: consume_player(state, &data.player),
                permission: data.permission,
                result: data.permission_result,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerMoveEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerMoveEvent(PlayerMoveEventData {
            player,
            from_position: to_wasm_position(self.from),
            to_position: to_wasm_position(self.to),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerMoveEvent(data) => Self {
                player: consume_player(state, &data.player),
                from: from_wasm_position(data.from_position),
                to: from_wasm_position(data.to_position),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerTeleportEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerTeleportEvent(PlayerTeleportEventData {
            player,
            from_position: to_wasm_position(self.from),
            to_position: to_wasm_position(self.to),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerTeleportEvent(data) => Self {
                player: consume_player(state, &data.player),
                from: from_wasm_position(data.from_position),
                to: from_wasm_position(data.to_position),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerChangeWorldEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let previous_world = state
            .add_world(self.previous_world.clone())
            .expect("添加世界资源失败");
        let new_world = state
            .add_world(self.new_world.clone())
            .expect("添加世界资源失败");

        Event::PlayerChangeWorldEvent(PlayerChangeWorldEventData {
            player,
            previous_world,
            new_world,
            position: to_wasm_position(self.position),
            yaw: self.yaw,
            pitch: self.pitch,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerChangeWorldEvent(data) => Self {
                player: consume_player(state, &data.player),
                previous_world: consume_world(state, &data.previous_world),
                new_world: consume_world(state, &data.new_world),
                position: from_wasm_position(data.position),
                yaw: data.yaw,
                pitch: data.pitch,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerRespawnEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let previous_world = state
            .add_world(self.previous_world.clone())
            .expect("添加世界资源失败");
        let respawned_world = state
            .add_world(self.respawned_world.clone())
            .expect("添加世界资源失败");

        Event::PlayerRespawnEvent(PlayerRespawnEventData {
            player,
            previous_world,
            respawned_world,
            position: to_wasm_position(self.position),
            yaw: self.yaw,
            pitch: self.pitch,
            alive: self.alive,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerRespawnEvent(data) => Self {
                player: consume_player(state, &data.player),
                previous_world: consume_world(state, &data.previous_world),
                respawned_world: consume_world(state, &data.respawned_world),
                position: from_wasm_position(data.position),
                yaw: data.yaw,
                pitch: data.pitch,
                alive: data.alive,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerExpChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerExpChangeEvent(PlayerExpChangeEventData {
            player,
            amount: self.amount,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerExpChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                amount: data.amount,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerItemHeldEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerItemHeldEvent(PlayerItemHeldEventData {
            player,
            previous_slot: self.previous_slot,
            new_slot: self.new_slot,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemHeldEvent(data) => Self {
                player: consume_player(state, &data.player),
                previous_slot: data.previous_slot,
                new_slot: data.new_slot,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerChangedMainHandEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerChangedMainHandEvent(PlayerChangedMainHandEventData {
            player,
            main_hand: to_wasm_hand(self.main_hand),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerChangedMainHandEvent(data) => Self {
                player: consume_player(state, &data.player),
                main_hand: from_wasm_hand(data.main_hand),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerGamemodeChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerGamemodeChangeEvent(PlayerGamemodeChangeEventData {
            player,
            previous_gamemode: to_wasm_game_mode(self.previous_gamemode),
            new_gamemode: to_wasm_game_mode(self.new_gamemode),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerGamemodeChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                previous_gamemode: from_wasm_game_mode(data.previous_gamemode),
                new_gamemode: from_wasm_game_mode(data.new_gamemode),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerCustomPayloadEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerCustomPayloadEvent(PlayerCustomPayloadEventData {
            player,
            channel: self.channel.clone(),
            data: self.data.to_vec(),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerCustomPayloadEvent(data) => Self {
                player: consume_player(state, &data.player),
                channel: data.channel,
                data: Bytes::from(data.data),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerFishEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerFishEvent(PlayerFishEventData {
            player,
            caught_uuid: self.caught_uuid.as_ref().map(WitUuid::to_wit),
            caught_type: self.caught_type.clone(),
            hook_uuid: WitUuid::to_wit(&self.hook_uuid),
            state: to_wasm_fish_state(self.state),
            hand: to_wasm_hand(self.hand),
            exp_to_drop: self.exp_to_drop,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerFishEvent(data) => Self {
                player: consume_player(state, &data.player),
                caught_uuid: data.caught_uuid.map(|id| WitUuid::from_wit(&id)),
                caught_type: data.caught_type,
                hook_uuid: WitUuid::from_wit(&data.hook_uuid),
                state: from_wasm_fish_state(data.state),
                hand: from_wasm_hand(data.hand),
                exp_to_drop: data.exp_to_drop,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerEggThrowEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerEggThrowEvent(PlayerEggThrowEventData {
            player,
            egg_uuid: WitUuid::to_wit(&self.egg_uuid),
            hatching: self.hatching,
            num_hatches: self.num_hatches,
            hatching_type: to_wasm_entity_type(self.hatching_type),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerEggThrowEvent(data) => Self {
                player: consume_player(state, &data.player),
                egg_uuid: WitUuid::from_wit(&data.egg_uuid),
                hatching: data.hatching,
                num_hatches: data.num_hatches,
                hatching_type: from_wasm_entity_type(&data.hatching_type),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerInteractUnknownEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerInteractUnknownEntityEvent(PlayerInteractUnknownEntityEventData {
            player,
            entity_id: self.entity_id,
            action: to_wasm_entity_interaction_action(self.action),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInteractUnknownEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                action: from_wasm_entity_interaction_action(data.action),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

const fn to_wasm_interact_action(action: &InteractAction) -> WasmInteractAction {
    match action {
        InteractAction::LeftClickBlock => WasmInteractAction::LeftClickBlock,
        InteractAction::LeftClickAir => WasmInteractAction::LeftClickAir,
        InteractAction::RightClickAir => WasmInteractAction::RightClickAir,
        InteractAction::RightClickBlock => WasmInteractAction::RightClickBlock,
    }
}

const fn from_wasm_interact_action(action: WasmInteractAction) -> InteractAction {
    match action {
        WasmInteractAction::LeftClickBlock => InteractAction::LeftClickBlock,
        WasmInteractAction::LeftClickAir => InteractAction::LeftClickAir,
        WasmInteractAction::RightClickAir => InteractAction::RightClickAir,
        WasmInteractAction::RightClickBlock => InteractAction::RightClickBlock,
    }
}

impl ToFromWasmEvent for PlayerInteractEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerInteractEvent(PlayerInteractEventData {
            player,
            action: to_wasm_interact_action(&self.action),
            clicked_pos: self.clicked_pos.map(to_wasm_block_position),
            block: self.block.name.to_string(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInteractEvent(data) => Self {
                player: consume_player(state, &data.player),
                action: from_wasm_interact_action(data.action),
                clicked_pos: data.clicked_pos.map(from_wasm_block_position),
                block: from_wasm_block_name(&data.block),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerToggleSneakEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerToggleSneakEvent(PlayerToggleSneakEventData {
            player,
            is_sneaking: self.is_sneaking,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerToggleSneakEvent(data) => Self {
                player: consume_player(state, &data.player),
                is_sneaking: data.is_sneaking,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerToggleFlightEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerToggleFlightEvent(PlayerToggleFlightEventData {
            player,
            is_flying: self.is_flying,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerToggleFlightEvent(data) => Self {
                player: consume_player(state, &data.player),
                is_flying: data.is_flying,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerToggleSprintEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerToggleSprintEvent(PlayerToggleSprintEventData {
            player,
            is_sprinting: self.is_sprinting,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerToggleSprintEvent(data) => Self {
                player: consume_player(state, &data.player),
                is_sprinting: data.is_sprinting,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerInteractEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerInteractEntityEvent(PlayerInteractEntityEventData {
            player,
            entity_id: self.target.get_entity().entity_id,
            action: to_wasm_entity_interaction_action(self.action),
            sneaking: self.sneaking,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInteractEntityEvent(data) => {
                let player = consume_player(state, &data.player);
                let target = player
                    .world()
                    .get_entity_by_id(data.entity_id)
                    .expect("未找到实体");
                Self {
                    player,
                    target,
                    action: from_wasm_entity_interaction_action(data.action),
                    target_position: None,
                    sneaking: data.sneaking,
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::player::player_item_consume::PlayerItemConsumeEvent
{
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerItemConsumeEvent(PlayerItemConsumeEventData {
            player,
            item_name: self.item_name.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemConsumeEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_name: data.item_name,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::player::player_item_damage::PlayerItemDamageEvent
{
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerItemDamageEvent(PlayerItemDamageEventData {
            player,
            item_name: self.item_name.clone(),
            damage: self.damage,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemDamageEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_name: data.item_name,
                damage: data.damage,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::player::player_drop_item::PlayerDropItemEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerDropItemEvent(PlayerDropItemEventData {
            player,
            item_name: self.item_name.clone(),
            count: self.count,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerDropItemEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_name: data.item_name,
                count: data.count,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::player::player_bed::PlayerBedEnterEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerBedEnterEvent(PlayerBedEnterEventData {
            player,
            bed_pos: to_wasm_block_position(self.bed_pos),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerBedEnterEvent(data) => Self {
                player: consume_player(state, &data.player),
                bed_pos: from_wasm_block_position(data.bed_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::player::player_bed::PlayerBedLeaveEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerBedLeaveEvent(PlayerBedLeaveEventData {
            player,
            bed_pos: to_wasm_block_position(self.bed_pos),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerBedLeaveEvent(data) => Self {
                player: consume_player(state, &data.player),
                bed_pos: from_wasm_block_position(data.bed_pos),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::player::player_bucket::PlayerBucketEmptyEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerBucketEmptyEvent(PlayerBucketEmptyEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            bucket: self.bucket.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerBucketEmptyEvent(data) => Self {
                player: consume_player(state, &data.player),
                block_pos: from_wasm_block_position(data.block_pos),
                bucket: data.bucket,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::player::player_bucket::PlayerBucketFillEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        Event::PlayerBucketFillEvent(PlayerBucketFillEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            bucket: self.bucket.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerBucketFillEvent(data) => Self {
                player: consume_player(state, &data.player),
                block_pos: from_wasm_block_position(data.block_pos),
                bucket: data.bucket,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for AsyncPlayerChatEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let format = state
            .add_text_component(self.format.clone())
            .expect("添加文本组件资源失败");
        Event::AsyncPlayerChatEvent(AsyncPlayerChatEventData {
            player,
            message: self.message.clone(),
            format,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        if !matches!(&event, Event::AsyncPlayerChatEvent(_)) {
            cleanup_event(&event, state);
            return;
        }

        let returned = Self::from_wasm_event(event, state);
        self.cancelled = returned.cancelled;
        self.message = returned.message;
        self.format = returned.format;
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncPlayerChatEvent(data) => Self {
                player: consume_player(state, &data.player),
                message: data.message,
                format: consume_text_component(state, &data.format),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for AsyncPlayerPreLoginEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let kick_message = state
            .add_text_component(self.kick_message.clone())
            .expect("添加文本组件资源失败");
        Event::AsyncPlayerPreLoginEvent(AsyncPlayerPreLoginEventData {
            player_name: self.player_name.clone(),
            player_uuid: self.player_uuid.to_string(),
            ip_address: self.ip_address.to_string(),
            kick_message,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        if !matches!(&event, Event::AsyncPlayerPreLoginEvent(_)) {
            cleanup_event(&event, state);
            return;
        }

        let returned = Self::from_wasm_event(event, state);
        self.cancelled = returned.cancelled;
        self.kick_message = returned.kick_message;
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncPlayerPreLoginEvent(data) => Self {
                player_name: data.player_name,
                player_uuid: data.player_uuid.parse().unwrap_or_default(),
                ip_address: data.ip_address.parse().unwrap_or_else(|_| {
                    std::net::SocketAddr::new(
                        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                        0,
                    )
                }),
                kick_message: consume_text_component(state, &data.kick_message),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerAdvancementDoneEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerAdvancementDoneEvent(PlayerAdvancementDoneEventData {
            player,
            advancement_id: self.advancement_id.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerAdvancementDoneEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerAdvancementDoneEvent(data) => Self {
                player: consume_player(state, &data.player),
                advancement_id: data.advancement_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerAnimationEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let animation_type = match self.animation_type {
            PlayerAnimationType::ArmSwingOff => "ArmSwingOff".to_string(),
            PlayerAnimationType::LeaveBed => "LeaveBed".to_string(),
            PlayerAnimationType::ArmSwingMain => "ArmSwingMain".to_string(),
        };
        Event::PlayerAnimationEvent(PlayerAnimationEventData {
            player,
            animation_type,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerAnimationEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerAnimationEvent(data) => Self {
                player: consume_player(state, &data.player),
                animation_type: match data.animation_type.as_str() {
                    "ArmSwingOff" => PlayerAnimationType::ArmSwingOff,
                    "LeaveBed" => PlayerAnimationType::LeaveBed,
                    _ => PlayerAnimationType::ArmSwingMain,
                },
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerArmorStandManipulateEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerArmorStandManipulateEvent(PlayerArmorStandManipulateEventData {
            player,
            armor_stand_id: self.armor_stand_id,
            slot: self.slot,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerArmorStandManipulateEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerArmorStandManipulateEvent(data) => Self {
                player: consume_player(state, &data.player),
                armor_stand_id: data.armor_stand_id,
                slot: data.slot,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerBucketEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerBucketEntityEvent(PlayerBucketEntityEventData {
            player,
            entity_id: self.entity_id,
            bucket_item: self.bucket_item.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerBucketEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerBucketEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                bucket_item: data.bucket_item,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerChangedWorldEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let from_world = state
            .add_world(self.from_world.clone())
            .expect("添加世界资源失败");
        let to_world = state
            .add_world(self.to_world.clone())
            .expect("添加世界资源失败");
        Event::PlayerChangedWorldEvent(PlayerChangedWorldEventData {
            player,
            from_world,
            to_world,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerChangedWorldEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerChangedWorldEvent(data) => Self {
                player: consume_player(state, &data.player),
                from_world: consume_world(state, &data.from_world),
                to_world: consume_world(state, &data.to_world),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerChannelEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerChannelEvent(PlayerChannelEventData {
            player,
            channel: self.channel.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerChannelEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerChannelEvent(data) => Self {
                player: consume_player(state, &data.player),
                channel: data.channel,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerCommandPreprocessEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerCommandPreprocessEvent(PlayerCommandPreprocessEventData {
            player,
            command: self.command.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerCommandPreprocessEvent(data) = event {
            self.cancelled = data.cancelled;
            self.command = data.command;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerCommandPreprocessEvent(data) => Self {
                player: consume_player(state, &data.player),
                command: data.command,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerEditBookEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerEditBookEvent(PlayerEditBookEventData {
            player,
            slot: self.slot,
            pages: self.pages.clone(),
            title: self.title.clone(),
            signing: self.signing,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerEditBookEvent(data) = event {
            self.cancelled = data.cancelled;
            self.pages = data.pages;
            self.title = data.title;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerEditBookEvent(data) => Self {
                player: consume_player(state, &data.player),
                slot: data.slot,
                pages: data.pages,
                title: data.title,
                signing: data.signing,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerElytraBoostEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerElytraBoostEvent(PlayerElytraBoostEventData {
            player,
            firework_id: self.firework_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerElytraBoostEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerElytraBoostEvent(data) => Self {
                player: consume_player(state, &data.player),
                firework_id: data.firework_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerExpCooldownChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerExpCooldownChangeEvent(PlayerExpCooldownChangeEventData {
            player,
            new_cooldown: self.new_cooldown,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerExpCooldownChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_cooldown = data.new_cooldown;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerExpCooldownChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                new_cooldown: data.new_cooldown,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerHarvestBlockEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let harvested_items = self
            .harvested_items
            .iter()
            .map(|i| {
                state
                    .add_item_stack(Arc::new(Mutex::new(i.clone())))
                    .expect("添加物品堆资源失败")
            })
            .collect();
        Event::PlayerHarvestBlockEvent(PlayerHarvestBlockEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            harvested_items,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerHarvestBlockEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerHarvestBlockEvent(_) => {
                panic!("无法从 WASM 构造 PlayerHarvestBlockEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerHideEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerHideEntityEvent(PlayerHideEntityEventData {
            player,
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerHideEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerHideEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerItemBreakEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerItemBreakEvent(PlayerItemBreakEventData {
            player,
            item_name: self.item_name.clone(),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemBreakEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_name: data.item_name,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerItemMendEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerItemMendEvent(PlayerItemMendEventData {
            player,
            item_name: self.item_name.clone(),
            repair_amount: self.repair_amount,
            exp_consumed: self.exp_consumed,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerItemMendEvent(data) = event {
            self.cancelled = data.cancelled;
            self.repair_amount = data.repair_amount;
            self.exp_consumed = data.exp_consumed;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemMendEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_name: data.item_name,
                repair_amount: data.repair_amount,
                exp_consumed: data.exp_consumed,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerKickEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerKickEvent(PlayerKickEventData {
            player,
            reason: self.reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerKickEvent(data) = event {
            self.cancelled = data.cancelled;
            self.reason = data.reason;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerKickEvent(data) => Self {
                player: consume_player(state, &data.player),
                reason: data.reason,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLeashEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerLeashEntityEvent(PlayerLeashEntityEventData {
            player,
            entity_id: self.entity_id,
            holder_id: self.holder_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerLeashEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLeashEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                holder_id: data.holder_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLevelChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerLevelChangeEvent(PlayerLevelChangeEventData {
            player,
            old_level: self.old_level,
            new_level: self.new_level,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLevelChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                old_level: data.old_level,
                new_level: data.new_level,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLocaleChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerLocaleChangeEvent(PlayerLocaleChangeEventData {
            player,
            new_locale: self.new_locale.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerLocaleChangeEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLocaleChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                new_locale: data.new_locale,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerNameEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let name = state
            .add_text_component(self.name.clone())
            .expect("添加文本组件资源失败");
        Event::PlayerNameEntityEvent(PlayerNameEntityEventData {
            player,
            entity_id: self.entity_id,
            name,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        if !matches!(&event, Event::PlayerNameEntityEvent(_)) {
            cleanup_event(&event, state);
            return;
        }

        let returned = Self::from_wasm_event(event, state);
        self.cancelled = returned.cancelled;
        self.name = returned.name;
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerNameEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                name: consume_text_component(state, &data.name),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerOpenSignEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerOpenSignEvent(PlayerOpenSignEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            is_front: self.is_front,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerOpenSignEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerOpenSignEvent(data) => Self {
                player: consume_player(state, &data.player),
                block_pos: from_wasm_block_position(data.block_pos),
                is_front: data.is_front,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPortalEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerPortalEvent(PlayerPortalEventData {
            player,
            from_pos: to_wasm_block_position(self.from_pos),
            to_pos: self.to_pos.map(to_wasm_block_position),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerPortalEvent(data) = event {
            self.cancelled = data.cancelled;
            self.to_pos = data.to_pos.map(from_wasm_block_position);
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPortalEvent(data) => Self {
                player: consume_player(state, &data.player),
                from_pos: from_wasm_block_position(data.from_pos),
                to_pos: data.to_pos.map(from_wasm_block_position),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPreLoginEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let kick_message = state
            .add_text_component(self.kick_message.clone())
            .expect("添加文本组件资源失败");
        Event::PlayerPreLoginEvent(PlayerPreLoginEventData {
            player_name: self.player_name.clone(),
            player_uuid: self.player_uuid.to_string(),
            ip_address: self.ip_address.to_string(),
            kick_message,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        if !matches!(&event, Event::PlayerPreLoginEvent(_)) {
            cleanup_event(&event, state);
            return;
        }

        let returned = Self::from_wasm_event(event, state);
        self.cancelled = returned.cancelled;
        self.kick_message = returned.kick_message;
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPreLoginEvent(data) => Self {
                player_name: data.player_name,
                player_uuid: data.player_uuid.parse().unwrap_or_default(),
                ip_address: data.ip_address.parse().unwrap_or_else(|_| {
                    std::net::SocketAddr::new(
                        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                        0,
                    )
                }),
                kick_message: consume_text_component(state, &data.kick_message),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerRiptideEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerRiptideEvent(PlayerRiptideEventData {
            player,
            item_name: self.item_name.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerRiptideEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerRiptideEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_name: data.item_name,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerShearEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerShearEntityEvent(PlayerShearEntityEventData {
            player,
            entity_id: self.entity_id,
            hand: self.hand,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerShearEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerShearEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                hand: data.hand,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerShowEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerShowEntityEvent(PlayerShowEntityEventData {
            player,
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerShowEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerShowEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerSpawnChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerSpawnChangeEvent(PlayerSpawnChangeEventData {
            player,
            new_spawn: self.new_spawn.map(to_wasm_block_position),
            forced: self.forced,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerSpawnChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_spawn = data.new_spawn.map(from_wasm_block_position);
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerSpawnChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                new_spawn: data.new_spawn.map(from_wasm_block_position),
                forced: data.forced,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerStatisticIncrementEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerStatisticIncrementEvent(PlayerStatisticIncrementEventData {
            player,
            statistic_id: self.statistic_id.clone(),
            amount: self.amount,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerStatisticIncrementEvent(data) = event {
            self.cancelled = data.cancelled;
            self.amount = data.amount;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerStatisticIncrementEvent(data) => Self {
                player: consume_player(state, &data.player),
                statistic_id: data.statistic_id,
                amount: data.amount,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerSwapHandItemsEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerSwapHandsEvent(PlayerSwapHandsEventData {
            player,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerSwapHandsEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerSwapHandsEvent(data) => Self {
                player: consume_player(state, &data.player),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerTakeLecternBookEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let book = state
            .add_item_stack(Arc::new(Mutex::new(self.book.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerTakeLecternBookEvent(PlayerTakeLecternBookEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            book,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerTakeLecternBookEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerTakeLecternBookEvent(_) => {
                panic!("无法从 WASM 构造 PlayerTakeLecternBookEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerUnleashEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerUnleashEntityEvent(PlayerUnleashEntityEventData {
            player,
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerUnleashEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerUnleashEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerVelocityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerVelocityEvent(PlayerVelocityEventData {
            player,
            velocity: to_wasm_position(self.velocity),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerVelocityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.velocity = from_wasm_position(data.velocity);
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerVelocityEvent(data) => Self {
                player: consume_player(state, &data.player),
                velocity: from_wasm_position(data.velocity),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerInputEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerInputEvent(PlayerInputEventData {
            player,
            input: self.input.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerInputEvent(data) = event {
            self.cancelled = data.cancelled;
            self.input = data.input;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInputEvent(data) => Self {
                player: consume_player(state, &data.player),
                input: data.input,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerInteractAtEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerInteractAtEntityEvent(PlayerInteractAtEntityEventData {
            player,
            entity_id: self.entity_id,
            clicked_x: self.clicked_x,
            clicked_y: self.clicked_y,
            clicked_z: self.clicked_z,
            hand: self.hand,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerInteractAtEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInteractAtEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
                clicked_x: data.clicked_x,
                clicked_y: data.clicked_y,
                clicked_z: data.clicked_z,
                hand: data.hand,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLinksSendEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerLinksSendEvent(PlayerLinksSendEventData {
            player,
            links: self.links.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerLinksSendEvent(data) = event {
            self.cancelled = data.cancelled;
            self.links = data.links;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLinksSendEvent(data) => Self {
                player: consume_player(state, &data.player),
                links: data.links,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPickupArrowEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerPickupArrowEvent(PlayerPickupArrowEventData {
            player,
            arrow_id: self.arrow_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerPickupArrowEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPickupArrowEvent(data) => Self {
                player: consume_player(state, &data.player),
                arrow_id: data.arrow_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerRecipeBookClickEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerRecipeBookClickEvent(PlayerRecipeBookClickEventData {
            player,
            recipe_id: self.recipe_id.clone(),
            make_all: self.make_all,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerRecipeBookClickEvent(data) = event {
            self.cancelled = data.cancelled;
            self.recipe_id = data.recipe_id;
            self.make_all = data.make_all;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerRecipeBookClickEvent(data) => Self {
                player: consume_player(state, &data.player),
                recipe_id: data.recipe_id,
                make_all: data.make_all,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerRecipeBookSettingsChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerRecipeBookSettingsChangeEvent(PlayerRecipeBookSettingsChangeEventData {
            player,
            book_type: self.book_type.clone(),
            is_open: self.is_open,
            is_filtering: self.is_filtering,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerRecipeBookSettingsChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.book_type = data.book_type;
            self.is_open = data.is_open;
            self.is_filtering = data.is_filtering;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerRecipeBookSettingsChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                book_type: data.book_type,
                is_open: data.is_open,
                is_filtering: data.is_filtering,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerRecipeDiscoverEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerRecipeDiscoverEvent(PlayerRecipeDiscoverEventData {
            player,
            recipe_id: self.recipe_id.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerRecipeDiscoverEvent(data) = event {
            self.cancelled = data.cancelled;
            self.recipe_id = data.recipe_id;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerRecipeDiscoverEvent(data) => Self {
                player: consume_player(state, &data.player),
                recipe_id: data.recipe_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerRegisterChannelEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerRegisterChannelEvent(PlayerRegisterChannelEventData {
            player,
            channel: self.channel.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerRegisterChannelEvent(data) = event {
            self.cancelled = data.cancelled;
            self.channel = data.channel;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerRegisterChannelEvent(data) => Self {
                player: consume_player(state, &data.player),
                channel: data.channel,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerResourcePackStatusEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerResourcePackStatusEvent(PlayerResourcePackStatusEventData {
            player,
            pack_id: self.pack_id.clone(),
            status: self.status.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerResourcePackStatusEvent(data) = event {
            self.cancelled = data.cancelled;
            self.status = data.status;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerResourcePackStatusEvent(data) => Self {
                player: consume_player(state, &data.player),
                pack_id: data.pack_id,
                status: data.status,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerSpawnLocationEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerSpawnLocationEvent(PlayerSpawnLocationEventData {
            player,
            spawn_pos: to_wasm_position(self.spawn_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerSpawnLocationEvent(data) = event {
            self.cancelled = data.cancelled;
            self.spawn_pos = from_wasm_position(data.spawn_pos);
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerSpawnLocationEvent(data) => Self {
                player: consume_player(state, &data.player),
                spawn_pos: from_wasm_position(data.spawn_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerUnregisterChannelEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerUnregisterChannelEvent(PlayerUnregisterChannelEventData {
            player,
            channel: self.channel.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerUnregisterChannelEvent(data) = event {
            self.cancelled = data.cancelled;
            self.channel = data.channel;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerUnregisterChannelEvent(data) => Self {
                player: consume_player(state, &data.player),
                channel: data.channel,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

const fn to_wasm_item_frame_action(action: ItemFrameAction) -> WasmItemFrameAction {
    match action {
        ItemFrameAction::Place => WasmItemFrameAction::Place,
        ItemFrameAction::Remove => WasmItemFrameAction::Remove,
        ItemFrameAction::Rotate => WasmItemFrameAction::Rotate,
    }
}

const fn to_wasm_handshake_intention(intention: HandshakeIntention) -> WasmHandshakeIntention {
    match intention {
        HandshakeIntention::Status => WasmHandshakeIntention::Status,
        HandshakeIntention::Login => WasmHandshakeIntention::Login,
    }
}

const fn from_wasm_handshake_intention(intention: WasmHandshakeIntention) -> HandshakeIntention {
    match intention {
        WasmHandshakeIntention::Status => HandshakeIntention::Status,
        WasmHandshakeIntention::Login => HandshakeIntention::Login,
    }
}

const fn to_wasm_server_full_check_result(
    result: ServerFullCheckResult,
) -> WasmServerFullCheckResult {
    match result {
        ServerFullCheckResult::Allowed => WasmServerFullCheckResult::Allowed,
        ServerFullCheckResult::Denied => WasmServerFullCheckResult::Denied,
    }
}

const fn from_wasm_server_full_check_result(
    result: WasmServerFullCheckResult,
) -> ServerFullCheckResult {
    match result {
        WasmServerFullCheckResult::Allowed => ServerFullCheckResult::Allowed,
        WasmServerFullCheckResult::Denied => ServerFullCheckResult::Denied,
    }
}

impl ToFromWasmEvent for PlayerBedFailEnterEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerBedFailEnterEvent(PlayerBedFailEnterEventData {
            player,
            bed_pos: to_wasm_block_position(self.bed_pos),
            fail_reason: self.fail_reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerBedFailEnterEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerBedFailEnterEvent(data) => Self {
                player: consume_player(state, &data.player),
                bed_pos: from_wasm_block_position(data.bed_pos),
                fail_reason: data.fail_reason,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerDeepSleepEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerDeepSleepEvent(PlayerDeepSleepEventData { player })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerDeepSleepEvent(data) => Self {
                player: consume_player(state, &data.player),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerInventorySlotChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let old_item = self.old_item.as_ref().map(|stack| {
            state
                .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                .expect("添加物品堆资源失败")
        });
        let new_item = self.new_item.as_ref().map(|stack| {
            state
                .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                .expect("添加物品堆资源失败")
        });
        Event::PlayerInventorySlotChangeEvent(PlayerInventorySlotChangeEventData {
            player,
            slot: self.slot,
            old_item,
            new_item,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInventorySlotChangeEvent(_) => {
                panic!("无法从 WASM 构造 PlayerInventorySlotChangeEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerItemCooldownEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerItemCooldownEvent(PlayerItemCooldownEventData {
            player,
            item_type: self.item_type.clone(),
            cooldown: self.cooldown,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerItemCooldownEvent(data) = event {
            self.cancelled = data.cancelled;
            self.cooldown = data.cooldown;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemCooldownEvent(data) => Self {
                player: consume_player(state, &data.player),
                item_type: data.item_type,
                cooldown: data.cooldown,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerItemGroupCooldownEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerItemGroupCooldownEvent(PlayerItemGroupCooldownEventData {
            player,
            cooldown_group: self.cooldown_group.clone(),
            cooldown: self.cooldown,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerItemGroupCooldownEvent(data) = event {
            self.cancelled = data.cancelled;
            self.cooldown = data.cooldown;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemGroupCooldownEvent(data) => Self {
                player: consume_player(state, &data.player),
                cooldown_group: data.cooldown_group,
                cooldown: data.cooldown,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerSwapWithEquipmentSlotEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let equipped_item = self.equipped_item.as_ref().map(|stack| {
            state
                .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                .expect("添加物品堆资源失败")
        });
        let cursor_item = self.cursor_item.as_ref().map(|stack| {
            state
                .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                .expect("添加物品堆资源失败")
        });
        Event::PlayerSwapWithEquipmentSlotEvent(PlayerSwapWithEquipmentSlotEventData {
            player,
            slot: self.slot.clone(),
            equipped_item,
            cursor_item,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerSwapWithEquipmentSlotEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerSwapWithEquipmentSlotEvent(_) => {
                panic!("无法从 WASM 构造 PlayerSwapWithEquipmentSlotEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerArmorChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let old_item = self.old_item.as_ref().map(|stack| {
            state
                .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                .expect("添加物品堆资源失败")
        });
        let new_item = self.new_item.as_ref().map(|stack| {
            state
                .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                .expect("添加物品堆资源失败")
        });
        Event::PlayerArmorChangeEvent(PlayerArmorChangeEventData {
            player,
            slot: self.slot.clone(),
            old_item,
            new_item,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerArmorChangeEvent(_) => {
                panic!("无法从 WASM 构造 PlayerArmorChangeEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerAttackEntityCooldownResetEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerAttackEntityCooldownResetEvent(PlayerAttackEntityCooldownResetEventData {
            player,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerAttackEntityCooldownResetEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerAttackEntityCooldownResetEvent(data) => Self {
                player: consume_player(state, &data.player),
                target_id: data.target_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PrePlayerAttackEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PrePlayerAttackEntityEvent(PrePlayerAttackEntityEventData {
            player,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PrePlayerAttackEntityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.target_id = data.target_id;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PrePlayerAttackEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                target_id: data.target_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerJumpEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerJumpEvent(PlayerJumpEventData {
            player,
            from_position: to_wasm_position(self.from_position),
            to_position: to_wasm_position(self.to_position),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerJumpEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerJumpEvent(data) => Self {
                player: consume_player(state, &data.player),
                from_position: from_wasm_position(data.from_position),
                to_position: from_wasm_position(data.to_position),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPickupExperienceEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerPickupExperienceEvent(PlayerPickupExperienceEventData {
            player,
            orb_id: self.orb_id,
            amount: self.amount,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerPickupExperienceEvent(data) = event {
            self.cancelled = data.cancelled;
            self.amount = data.amount;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPickupExperienceEvent(data) => Self {
                player: consume_player(state, &data.player),
                orb_id: data.orb_id,
                amount: data.amount,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPostRespawnEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerPostRespawnEvent(PlayerPostRespawnEventData {
            player,
            respawn_location: to_wasm_position(self.respawn_location),
            is_bed_spawn: self.is_bed_spawn,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPostRespawnEvent(data) => Self {
                player: consume_player(state, &data.player),
                respawn_location: from_wasm_position(data.respawn_location),
                is_bed_spawn: data.is_bed_spawn,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerClientOptionsChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerClientOptionsChangeEvent(PlayerClientOptionsChangeEventData {
            player,
            locale: self.locale.clone(),
            view_distance: self.view_distance,
            chat_visibility: self.chat_visibility.clone(),
            chat_colors: self.chat_colors,
            main_hand: self.main_hand.clone(),
            skin_parts: self.skin_parts,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerClientOptionsChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                locale: data.locale,
                view_distance: data.view_distance,
                chat_visibility: data.chat_visibility,
                chat_colors: data.chat_colors,
                main_hand: data.main_hand,
                skin_parts: data.skin_parts,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerTrackEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerTrackEntityEvent(PlayerTrackEntityEventData {
            player,
            entity_id: self.entity_id,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerTrackEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerUntrackEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerUntrackEntityEvent(PlayerUntrackEntityEventData {
            player,
            entity_id: self.entity_id,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerUntrackEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                entity_id: data.entity_id,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerStopUsingItemEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let item = state
            .add_item_stack(Arc::new(Mutex::new(self.item.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerStopUsingItemEvent(PlayerStopUsingItemEventData { player, item })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerStopUsingItemEvent(_) => {
                panic!("无法从 WASM 构造 PlayerStopUsingItemEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerReadyArrowEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let bow = state
            .add_item_stack(Arc::new(Mutex::new(self.bow.clone())))
            .expect("添加物品堆资源失败");
        let arrow = state
            .add_item_stack(Arc::new(Mutex::new(self.arrow.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerReadyArrowEvent(PlayerReadyArrowEventData {
            player,
            bow,
            arrow,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerReadyArrowEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerReadyArrowEvent(_) => {
                panic!("无法从 WASM 构造 PlayerReadyArrowEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerItemFrameChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let item = state
            .add_item_stack(Arc::new(Mutex::new(self.item.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerItemFrameChangeEvent(PlayerItemFrameChangeEventData {
            player,
            frame_id: self.frame_id,
            item,
            action: to_wasm_item_frame_action(self.action),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerItemFrameChangeEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerItemFrameChangeEvent(_) => {
                panic!("无法从 WASM 构造 PlayerItemFrameChangeEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerFlowerPotManipulateEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let item = state
            .add_item_stack(Arc::new(Mutex::new(self.item.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerFlowerPotManipulateEvent(PlayerFlowerPotManipulateEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            item,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerFlowerPotManipulateEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerFlowerPotManipulateEvent(_) => {
                panic!("无法从 WASM 构造 PlayerFlowerPotManipulateEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerInsertLecternBookEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let book = state
            .add_item_stack(Arc::new(Mutex::new(self.book.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerInsertLecternBookEvent(PlayerInsertLecternBookEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            book,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerInsertLecternBookEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerInsertLecternBookEvent(_) => {
                panic!("无法从 WASM 构造 PlayerInsertLecternBookEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLecternPageChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let book = state
            .add_item_stack(Arc::new(Mutex::new(self.book.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerLecternPageChangeEvent(PlayerLecternPageChangeEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            book,
            new_page: self.new_page,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerLecternPageChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_page = data.new_page;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLecternPageChangeEvent(_) => {
                panic!("无法从 WASM 构造 PlayerLecternPageChangeEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerLoomPatternSelectEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerLoomPatternSelectEvent(PlayerLoomPatternSelectEventData {
            player,
            pattern: self.pattern.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerLoomPatternSelectEvent(data) = event {
            self.cancelled = data.cancelled;
            self.pattern = data.pattern;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerLoomPatternSelectEvent(data) => Self {
                player: consume_player(state, &data.player),
                pattern: data.pattern,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPickBlockEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let result = state
            .add_item_stack(Arc::new(Mutex::new(self.result.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerPickBlockEvent(PlayerPickBlockEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            result,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerPickBlockEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPickBlockEvent(_) => {
                panic!("无法从 WASM 构造 PlayerPickBlockEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPickEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let result = state
            .add_item_stack(Arc::new(Mutex::new(self.result.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerPickEntityEvent(PlayerPickEntityEventData {
            player,
            entity_id: self.entity_id,
            result,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerPickEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPickEntityEvent(_) => {
                panic!("无法从 WASM 构造 PlayerPickEntityEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerPurchaseEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let ingredients = self
            .ingredients
            .iter()
            .map(|stack| {
                state
                    .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                    .expect("添加物品堆资源失败")
            })
            .collect();
        let result = state
            .add_item_stack(Arc::new(Mutex::new(self.result.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerPurchaseEvent(PlayerPurchaseEventData {
            player,
            merchant_id: self.merchant_id,
            ingredients,
            result,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerPurchaseEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerPurchaseEvent(_) => {
                panic!("无法从 WASM 构造 PlayerPurchaseEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerTradeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let ingredients = self
            .ingredients
            .iter()
            .map(|stack| {
                state
                    .add_item_stack(Arc::new(Mutex::new(stack.clone())))
                    .expect("添加物品堆资源失败")
            })
            .collect();
        let result = state
            .add_item_stack(Arc::new(Mutex::new(self.result.clone())))
            .expect("添加物品堆资源失败");
        Event::PlayerTradeEvent(PlayerTradeEventData {
            player,
            merchant_id: self.merchant_id,
            ingredients,
            result,
            villager_experience: self.villager_experience,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerTradeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.villager_experience = data.villager_experience;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerTradeEvent(_) => {
                panic!("无法从 WASM 构造 PlayerTradeEvent")
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerServerFullCheckEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PlayerServerFullCheckEvent(PlayerServerFullCheckEventData {
            player_name: self.player_name.clone(),
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            result: to_wasm_server_full_check_result(self.result),
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerServerFullCheckEvent(data) = event {
            self.result = from_wasm_server_full_check_result(data.result);
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerServerFullCheckEvent(data) => Self {
                player_name: data.player_name,
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                result: from_wasm_server_full_check_result(data.result),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerSignCommandPreprocessEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerSignCommandPreprocessEvent(PlayerSignCommandPreprocessEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            command: self.command.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerSignCommandPreprocessEvent(data) = event {
            self.cancelled = data.cancelled;
            self.command = data.command;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerSignCommandPreprocessEvent(data) => Self {
                player: consume_player(state, &data.player),
                block_pos: from_wasm_block_position(data.block_pos),
                command: data.command,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerStonecutterRecipeSelectEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerStonecutterRecipeSelectEvent(PlayerStonecutterRecipeSelectEventData {
            player,
            recipe_id: self.recipe_id.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerStonecutterRecipeSelectEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerStonecutterRecipeSelectEvent(data) => Self {
                player: consume_player(state, &data.player),
                recipe_id: data.recipe_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for UncheckedSignChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::UncheckedSignChangeEvent(UncheckedSignChangeEventData {
            player,
            block_pos: to_wasm_block_position(self.block_pos),
            lines: self.lines.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::UncheckedSignChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.lines = data.lines;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::UncheckedSignChangeEvent(data) => Self {
                player: consume_player(state, &data.player),
                block_pos: from_wasm_block_position(data.block_pos),
                lines: data.lines,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerHandshakeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PlayerHandshakeEvent(PlayerHandshakeEventData {
            ip_address: self.ip_address.clone(),
            hostname: self.hostname.clone(),
            protocol_version: self.protocol_version,
            intention: to_wasm_handshake_intention(self.intention),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerHandshakeEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerHandshakeEvent(data) => Self {
                ip_address: data.ip_address,
                hostname: data.hostname,
                protocol_version: data.protocol_version,
                intention: from_wasm_handshake_intention(data.intention),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerConnectionCloseEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PlayerConnectionCloseEvent(PlayerConnectionCloseEventData {
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            player_name: self.player_name.clone(),
            ip_address: self.ip_address.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerConnectionCloseEvent(data) => Self {
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                player_name: data.player_name,
                ip_address: data.ip_address,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for AsyncTabCompleteEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let sender = self
            .sender
            .as_ref()
            .map(|player| state.add_player(player.clone()).expect("添加玩家资源失败"));
        Event::AsyncTabCompleteEvent(AsyncTabCompleteEventData {
            sender,
            buffer: self.buffer.clone(),
            completions: self.completions.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::AsyncTabCompleteEvent(data) = event {
            self.cancelled = data.cancelled;
            self.completions = data.completions;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncTabCompleteEvent(data) => Self {
                sender: data.sender.map(|sender| consume_player(state, &sender)),
                buffer: data.buffer,
                completions: data.completions,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for AsyncPlayerSendSuggestionsEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::AsyncPlayerSendSuggestionsEvent(AsyncPlayerSendSuggestionsEventData {
            player,
            buffer: self.buffer.clone(),
            suggestions: self.suggestions.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::AsyncPlayerSendSuggestionsEvent(data) = event {
            self.cancelled = data.cancelled;
            self.suggestions = data.suggestions;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncPlayerSendSuggestionsEvent(data) => Self {
                player: consume_player(state, &data.player),
                buffer: data.buffer,
                suggestions: data.suggestions,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for FillProfileEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::FillProfileEvent(FillProfileEventData {
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            player_name: self.player_name.clone(),
            properties: self.properties.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::FillProfileEvent(data) => Self {
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                player_name: data.player_name,
                properties: data.properties,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PreFillProfileEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PreFillProfileEvent(PreFillProfileEventData {
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            player_name: self.player_name.clone(),
            properties: self.properties.clone(),
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PreFillProfileEvent(data) = event {
            self.player_name = data.player_name;
            self.properties = data.properties;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PreFillProfileEvent(data) => Self {
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                player_name: data.player_name,
                properties: data.properties,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for LookupProfileEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::LookupProfileEvent(LookupProfileEventData {
            name: self.name.clone(),
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            player_name: self.player_name.clone(),
            properties: self.properties.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::LookupProfileEvent(data) => Self {
                name: data.name,
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                player_name: data.player_name,
                properties: data.properties,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PreLookupProfileEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PreLookupProfileEvent(PreLookupProfileEventData {
            name: self.name.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PreLookupProfileEvent(data) => Self { name: data.name },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for Gs4QueryEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::Gs4QueryEvent(Gs4QueryEventData {
            query_type: self.query_type.clone(),
            querier_address: self.querier_address.clone(),
            data: self.data.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::Gs4QueryEvent(data) => Self {
                query_type: data.query_type,
                querier_address: data.querier_address,
                data: data.data,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ClientTickEndEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::ClientTickEndEvent(ClientTickEndEventData { player })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ClientTickEndEvent(data) => Self {
                player: consume_player(state, &data.player),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerChunkUnloadEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let target_world = state
            .add_world(self.target_world.clone())
            .expect("添加世界资源失败");
        Event::PlayerChunkUnloadEvent(PlayerChunkUnloadEventData {
            player,
            target_world,
            chunk_x: self.chunk_x,
            chunk_z: self.chunk_z,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerChunkUnloadEvent(data) => Self {
                player: consume_player(state, &data.player),
                target_world: consume_world(state, &data.target_world),
                chunk_x: data.chunk_x,
                chunk_z: data.chunk_z,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerNaturallySpawnCreaturesEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerNaturallySpawnCreaturesEvent(PlayerNaturallySpawnCreaturesEventData {
            player,
            chunk_x: self.chunk_x,
            chunk_z: self.chunk_z,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerNaturallySpawnCreaturesEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerNaturallySpawnCreaturesEvent(data) => Self {
                player: consume_player(state, &data.player),
                chunk_x: data.chunk_x,
                chunk_z: data.chunk_z,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerStartSpectatingEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerStartSpectatingEntityEvent(PlayerStartSpectatingEntityEventData {
            player,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerStartSpectatingEntityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.target_id = data.target_id;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerStartSpectatingEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                target_id: data.target_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerStopSpectatingEntityEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerStopSpectatingEntityEvent(PlayerStopSpectatingEntityEventData {
            player,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerStopSpectatingEntityEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerStopSpectatingEntityEvent(data) => Self {
                player: consume_player(state, &data.player),
                target_id: data.target_id,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerTeleportEndGatewayEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        Event::PlayerTeleportEndGatewayEvent(PlayerTeleportEndGatewayEventData {
            player,
            gateway: to_wasm_block_position(self.gateway),
            from_position: to_wasm_position(self.from_position),
            to_position: to_wasm_position(self.to_position),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PlayerTeleportEndGatewayEvent(data) = event {
            self.cancelled = data.cancelled;
            self.to_position = from_wasm_position(data.to_position);
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerTeleportEndGatewayEvent(data) => Self {
                player: consume_player(state, &data.player),
                gateway: from_wasm_block_position(data.gateway),
                from_position: from_wasm_position(data.from_position),
                to_position: from_wasm_position(data.to_position),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}
