use crate::plugin::{
    loader::wasm::wasm_host::{
        state::PluginHostState,
        wit::v0_1::{
            events::{ToFromWasmEvent, cleanup_event, consume_text_component},
            generated_packets,
            papokin::plugin::event::{
                AsyncPlayerConnectionConfigureEventData, ClientboundPacket,
                CommandRegisteredEventData,
                ConnectionValidationResult as WitConnectionValidationResult, Event,
                MapInitializeEventData, PacketReceivedEventData, PacketSentEventData,
                PlayerConnectionInitialConfigureEventData, PlayerConnectionValidateLoginEventData,
                ProfileWhitelistVerifyEventData, ServerBroadcastEventData, ServerCommandEventData,
                ServerListPingAddress, ServerListPingEventData, ServerLoadEventData,
                ServerLoadType, ServerResourcesReloadedEventData, ServerTickEndEventData,
                ServerTickStartEventData, ServerboundPacket, WhitelistStateUpdateEventData,
                WhitelistStateUpdateStatus as WitWhitelistStateUpdateStatus,
                WhitelistToggleEventData, WhitelistVerifyResult as WitWhitelistVerifyResult,
            },
            papokin::plugin::uuid::Uuid as WitUuid,
            uuid::UuidExt,
        },
    },
    server::{
        async_player_connection_configure::AsyncPlayerConnectionConfigureEvent,
        command_registered::CommandRegisteredEvent,
        list_ping::ServerListPingEvent,
        map_initialize::MapInitializeEvent,
        packet::{PacketReceivedEvent, PacketSentEvent},
        player_connection_initial_configure::PlayerConnectionInitialConfigureEvent,
        player_connection_validate_login::{
            ConnectionValidationResult, PlayerConnectionValidateLoginEvent,
        },
        profile_whitelist_verify::{ProfileWhitelistVerifyEvent, WhitelistVerifyResult},
        server_broadcast::ServerBroadcastEvent,
        server_command::ServerCommandEvent,
        server_load::{LoadType, ServerLoadEvent},
        server_resources_reloaded::ServerResourcesReloadedEvent,
        server_tick_end::ServerTickEndEvent,
        server_tick_start::ServerTickStartEvent,
        whitelist_state_update::{WhitelistStateUpdateEvent, WhitelistStateUpdateStatus},
        whitelist_toggle::WhitelistToggleEvent,
    },
};

impl ToFromWasmEvent for PacketReceivedEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player_res = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        let version = self.player.client.version.load();
        let packet = generated_packets::deserialize_java_serverbound_packet(
            self.packet_id,
            &self.payload,
            version,
        )
        .map_or(ServerboundPacket::Unknown, ServerboundPacket::Java);

        Event::PacketReceivedEvent(PacketReceivedEventData {
            player: player_res,
            packet,
            packet_id: self.packet_id,
            raw_payload: self.payload.to_vec(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PacketReceivedEvent(data) = event {
            self.packet_id = data.packet_id;
            self.payload = data.raw_payload.into();
            self.cancelled = data.cancelled;
        }
    }
    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PacketReceivedEvent(_) => {
                // TODO: 如有需要，实现从 WIT 变体转换回原始形式。
                // 目前我们仅支持取消。
                panic!("在此简单实现中尚不支持从 WASM 修改数据包。");
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PacketSentEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player_res = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");

        let packet = generated_packets::clientbound_java_any_to_wit(self.packet.as_ref())
            .map_or(ClientboundPacket::Unknown, ClientboundPacket::Java);

        Event::PacketSentEvent(PacketSentEventData {
            player: player_res,
            packet,
            packet_id: self.packet_id,
            raw_payload: self.payload.iter().copied().collect(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PacketSentEvent(data) = event {
            self.payload = data.raw_payload.into();
            self.cancelled = data.cancelled;
        }
    }
    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PacketSentEvent(_) => {
                panic!("尚不支持从 WASM 修改数据包。");
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ServerCommandEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ServerCommandEvent(ServerCommandEventData {
            command: self.command.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerCommandEvent(data) => Self {
                command: data.command,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ServerBroadcastEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let message = state
            .add_text_component(self.message.clone())
            .expect("添加文本组件资源失败");
        let sender = state
            .add_text_component(self.sender.clone())
            .expect("添加文本组件资源失败");

        Event::ServerBroadcastEvent(ServerBroadcastEventData {
            message,
            sender,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerBroadcastEvent(data) => Self {
                message: consume_text_component(state, &data.message),
                sender: consume_text_component(state, &data.sender),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ServerListPingEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let motd = state
            .add_text_component(self.motd.clone())
            .expect("添加文本组件资源失败");

        Event::ServerListPingEvent(ServerListPingEventData {
            hostname: self.hostname().to_string(),
            address: ServerListPingAddress {
                host: self.address().host().to_string(),
                port: self.address().port(),
            },
            motd,
            max_players: self.max_players,
            num_players: self.num_players,
            favicon: self.favicon.clone(),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerListPingEvent(data) => Self {
                hostname: data.hostname,
                address: crate::plugin::api::events::server::list_ping::ServerListPingAddress::new(
                    data.address.host,
                    data.address.port,
                ),
                motd: consume_text_component(state, &data.motd),
                max_players: data.max_players,
                num_players: data.num_players,
                favicon: data.favicon,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        if !matches!(&event, Event::ServerListPingEvent(_)) {
            cleanup_event(&event, state);
            panic!("意外的事件类型");
        }

        let returned = Self::from_wasm_event(event, state);
        self.motd = returned.motd;
        self.max_players = returned.max_players;
        self.num_players = returned.num_players;
        self.favicon = returned.favicon;
    }
}

impl ToFromWasmEvent for ServerLoadEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ServerLoadEvent(ServerLoadEventData {
            load_type: match self.load_type {
                LoadType::Startup => ServerLoadType::Startup,
                LoadType::Reload => ServerLoadType::Reload,
            },
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerLoadEvent(data) => Self {
                load_type: match data.load_type {
                    ServerLoadType::Startup => LoadType::Startup,
                    ServerLoadType::Reload => LoadType::Reload,
                },
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ServerTickEndEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ServerTickEndEvent(ServerTickEndEventData {
            tick: self.tick,
            duration_nanos: self.duration_nanos,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerTickEndEvent(data) => Self {
                tick: data.tick,
                duration_nanos: data.duration_nanos,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ServerTickStartEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ServerTickStartEvent(ServerTickStartEventData { tick: self.tick })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerTickStartEvent(data) => Self { tick: data.tick },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for MapInitializeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::MapInitializeEvent(MapInitializeEventData {
            map_id: self.map_id,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::MapInitializeEvent(data) => Self {
                map_id: data.map_id,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}
const fn to_wasm_connection_validation_result(
    result: ConnectionValidationResult,
) -> WitConnectionValidationResult {
    match result {
        ConnectionValidationResult::Allowed => WitConnectionValidationResult::Allowed,
        ConnectionValidationResult::Denied => WitConnectionValidationResult::Denied,
    }
}

const fn from_wasm_connection_validation_result(
    result: WitConnectionValidationResult,
) -> ConnectionValidationResult {
    match result {
        WitConnectionValidationResult::Allowed => ConnectionValidationResult::Allowed,
        WitConnectionValidationResult::Denied => ConnectionValidationResult::Denied,
    }
}

const fn to_wasm_whitelist_verify_result(
    result: WhitelistVerifyResult,
) -> WitWhitelistVerifyResult {
    match result {
        WhitelistVerifyResult::Allowed => WitWhitelistVerifyResult::Allowed,
        WhitelistVerifyResult::Denied => WitWhitelistVerifyResult::Denied,
    }
}

const fn from_wasm_whitelist_verify_result(
    result: WitWhitelistVerifyResult,
) -> WhitelistVerifyResult {
    match result {
        WitWhitelistVerifyResult::Allowed => WhitelistVerifyResult::Allowed,
        WitWhitelistVerifyResult::Denied => WhitelistVerifyResult::Denied,
    }
}

const fn to_wasm_whitelist_state_update_status(
    status: WhitelistStateUpdateStatus,
) -> WitWhitelistStateUpdateStatus {
    match status {
        WhitelistStateUpdateStatus::Added => WitWhitelistStateUpdateStatus::Added,
        WhitelistStateUpdateStatus::Removed => WitWhitelistStateUpdateStatus::Removed,
    }
}

const fn from_wasm_whitelist_state_update_status(
    status: WitWhitelistStateUpdateStatus,
) -> WhitelistStateUpdateStatus {
    match status {
        WitWhitelistStateUpdateStatus::Added => WhitelistStateUpdateStatus::Added,
        WitWhitelistStateUpdateStatus::Removed => WhitelistStateUpdateStatus::Removed,
    }
}

impl ToFromWasmEvent for PlayerConnectionValidateLoginEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let kick_message = state
            .add_text_component(self.kick_message.clone())
            .expect("添加文本组件资源失败");
        Event::PlayerConnectionValidateLoginEvent(PlayerConnectionValidateLoginEventData {
            ip_address: self.ip_address.clone(),
            kick_message,
            result: to_wasm_connection_validation_result(self.result),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerConnectionValidateLoginEvent(data) => Self {
                ip_address: data.ip_address,
                kick_message: consume_text_component(state, &data.kick_message),
                result: from_wasm_connection_validation_result(data.result),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for AsyncPlayerConnectionConfigureEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::AsyncPlayerConnectionConfigureEvent(AsyncPlayerConnectionConfigureEventData {
            player_name: self.player_name.clone(),
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            first_join: self.first_join,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncPlayerConnectionConfigureEvent(data) => Self {
                player_name: data.player_name,
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                first_join: data.first_join,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for PlayerConnectionInitialConfigureEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PlayerConnectionInitialConfigureEvent(PlayerConnectionInitialConfigureEventData {
            player_name: self.player_name.clone(),
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            first_join: self.first_join,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerConnectionInitialConfigureEvent(data) => Self {
                player_name: data.player_name,
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                first_join: data.first_join,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ServerResourcesReloadedEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ServerResourcesReloadedEvent(ServerResourcesReloadedEventData {
            cause: self.cause.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ServerResourcesReloadedEvent(data) => Self { cause: data.cause },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for WhitelistStateUpdateEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::WhitelistStateUpdateEvent(WhitelistStateUpdateEventData {
            player_name: self.player_name.clone(),
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            status: to_wasm_whitelist_state_update_status(self.status),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WhitelistStateUpdateEvent(data) => Self {
                player_name: data.player_name,
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                status: from_wasm_whitelist_state_update_status(data.status),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for WhitelistToggleEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::WhitelistToggleEvent(WhitelistToggleEventData {
            enabled: self.enabled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WhitelistToggleEvent(data) => Self {
                enabled: data.enabled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for CommandRegisteredEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::CommandRegisteredEvent(CommandRegisteredEventData {
            command_label: self.command_label.clone(),
            plugin: self.plugin.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::CommandRegisteredEvent(data) => Self {
                command_label: data.command_label,
                plugin: data.plugin,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ProfileWhitelistVerifyEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let kick_message = state
            .add_text_component(self.kick_message.clone())
            .expect("添加文本组件资源失败");
        Event::ProfileWhitelistVerifyEvent(ProfileWhitelistVerifyEventData {
            player_uuid: WitUuid::to_wit(&self.player_uuid),
            player_name: self.player_name.clone(),
            kick_message,
            result: to_wasm_whitelist_verify_result(self.result),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ProfileWhitelistVerifyEvent(data) => Self {
                player_uuid: WitUuid::from_wit(&data.player_uuid),
                player_name: data.player_name,
                kick_message: consume_text_component(state, &data.kick_message),
                result: from_wasm_whitelist_verify_result(data.result),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::loader::wasm::wasm_host::state::TextComponentResource;
    use papokin_util::text::TextComponent;
    use wasmtime::component::Resource;

    #[test]
    fn server_list_ping_applies_and_consumes_returned_resources() {
        let mut state = PluginHostState::new();
        let original_motd = TextComponent::text("原始");
        let returned_motd = TextComponent::text("已返回");
        let mut event = ServerListPingEvent::new(
            "original.example".to_string(),
            "127.0.0.1:25565".parse().expect("测试地址应能解析"),
            original_motd,
            20,
            1,
            None,
        );
        let motd = state
            .add_text_component(returned_motd.clone())
            .expect("文本组件资源应已插入");
        let motd_rep = motd.rep();
        let returned = Event::ServerListPingEvent(ServerListPingEventData {
            hostname: "replacement.example".to_string(),
            address: ServerListPingAddress {
                host: "192.0.2.1".to_string(),
                port: 25_566,
            },
            motd,
            max_players: 40,
            num_players: 2,
            favicon: Some("data:image/png;base64,test".to_string()),
        });

        event.apply_wasm_event(returned, &mut state);

        assert_eq!(event.hostname(), "original.example");
        assert_eq!(event.address().host(), "127.0.0.1");
        assert_eq!(event.address().port(), 25_565);
        assert_eq!(event.motd, returned_motd);
        assert_eq!(event.max_players, 40);
        assert_eq!(event.num_players, 2);
        assert_eq!(event.favicon.as_deref(), Some("data:image/png;base64,test"));
        assert!(
            state
                .resource_table
                .get::<TextComponentResource>(&Resource::new_own(motd_rep))
                .is_err()
        );
    }
}
