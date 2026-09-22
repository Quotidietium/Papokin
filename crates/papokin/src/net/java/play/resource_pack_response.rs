#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::{PlayResourcePackResult, SPlayResourcePack};
use papokin_util::text::TextComponent;

use crate::plugin::api::events::player::player_resource_pack_status::PlayerResourcePackStatusEvent;

impl JavaClient {
    pub fn handle_play_resource_pack_response(
        &self,
        server: &Arc<Server>,
        player: &Arc<Player>,
        packet: &SPlayResourcePack,
    ) {
        let result = packet.response_result();
        debug!(
            "玩家 {} 对 {} 的资源包响应：{:?}",
            player.gameprofile.name, packet.uuid, result
        );

        let mut event = PlayerResourcePackStatusEvent::new(
            player.clone(),
            packet.uuid.to_string(),
            format!("{result:?}"),
        );
        server.plugin_manager.fire_blocking(server, &mut event);

        if server.advanced_config.resource_pack.java.force
            && (result == PlayResourcePackResult::Declined
                || result == PlayResourcePackResult::DownloadFail)
        {
            self.try_kick(&TextComponent::text(
                "你必须接受资源包才能在此服务器上游玩。",
            ));
        }
    }
}
