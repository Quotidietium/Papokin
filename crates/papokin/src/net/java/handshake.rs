use papokin_data::{
    packet::{CURRENT_MC_VERSION, LOWEST_SUPPORTED_MC_VERSION},
    translation,
};
use papokin_protocol::{ConnectionState, java::server::handshake::SHandShake};
use papokin_util::{text::TextComponent, version::JavaMinecraftVersion};
use tracing::debug;

use crate::{net::java::pending::PendingConnection, server::Server};
use std::sync::Arc;

impl PendingConnection {
    pub async fn handle_handshake(&mut self, server: &Arc<Server>, handshake: SHandShake) {
        let version = handshake.protocol_version.0 as u32;
        self.server_address = handshake.server_address.to_string();
        self.version
            .store(JavaMinecraftVersion::from_protocol(version));

        debug!("握手：下一个状态为 {:?}", &handshake.next_state);
        self.connection_state.store(handshake.next_state);
        if handshake.next_state == ConnectionState::Transfer
            && !server.basic_config.accepts_transfers
        {
            self.kick(TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_TRANSFERS_DISABLED,
                [],
            ))
            .await;
            return;
        }
        if self.connection_state.load() != ConnectionState::Status {
            let protocol = version;
            if protocol < LOWEST_SUPPORTED_MC_VERSION.protocol_version() as u32 {
                self.kick(TextComponent::translate(
                    translation::java::MULTIPLAYER_DISCONNECT_OUTDATED_CLIENT,
                    [TextComponent::text(CURRENT_MC_VERSION.to_string())],
                ))
                .await;
            } else if protocol > CURRENT_MC_VERSION.protocol_version() as u32 {
                self.kick(TextComponent::translate(
                    translation::java::MULTIPLAYER_DISCONNECT_OUTDATED_SERVER,
                    [TextComponent::text(CURRENT_MC_VERSION.to_string())],
                ))
                .await;
            }
        }
    }
}
