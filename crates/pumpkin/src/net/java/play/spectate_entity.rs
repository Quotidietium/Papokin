#[allow(clippy::wildcard_imports)]
use super::*;
use pumpkin_protocol::java::{client::play::CSetCamera, server::play::SSpectateEntity};
use pumpkin_util::GameMode;

impl JavaClient {
    pub fn handle_spectate_entity(
        &self,
        player: &Arc<Player>,
        server: &Server,
        packet: &SSpectateEntity,
    ) {
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();

        if player.gamemode.load() != GameMode::Spectator {
            return;
        }

        let world = player.world();
        if let Some(target) = world.get_entity_by_uuid(packet.target) {
            let target_pos = target.get_entity().pos.load();
            let target_yaw = target.get_entity().yaw.load();
            let target_pitch = target.get_entity().pitch.load();
            let target_id = target.get_entity().entity_id;

            if !Self::fire_start_spectating(player, server, target_id) {
                return;
            }

            player.camera_target_id.store(Some(target_id));
            player.try_send_client_packet(&CSetCamera::new(target_id.into()));

            player.request_teleport(target_pos, target_yaw, target_pitch);
        } else if let Some(target_player) = server.get_player_by_uuid(packet.target) {
            let target_pos = target_player.living_entity.entity.pos.load();
            let target_yaw = target_player.living_entity.entity.yaw.load();
            let target_pitch = target_player.living_entity.entity.pitch.load();
            let target_id = target_player.living_entity.entity.entity_id;

            if !Self::fire_start_spectating(player, server, target_id) {
                return;
            }

            player.camera_target_id.store(Some(target_id));
            player.try_send_client_packet(&CSetCamera::new(target_id.into()));

            player.request_teleport(target_pos, target_yaw, target_pitch);
        }
    }

    /// Fires `PlayerStartSpectatingEntityEvent`; returns `false` when the
    /// event was cancelled (the camera must not move).
    fn fire_start_spectating(player: &Arc<Player>, server: &Server, target_id: i32) -> bool {
        let world = player.world();
        let Some(server_arc) = world.server.upgrade() else {
            let _ = server;
            return true;
        };
        let mut event = crate::plugin::api::events::player::player_start_spectating_entity::PlayerStartSpectatingEntityEvent::new(
            player.clone(),
            target_id,
        );
        server_arc
            .plugin_manager
            .fire_blocking(&server_arc, &mut event);
        !event.cancelled
    }
}
