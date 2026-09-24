#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::{client::play::CSetCamera, server::play::SSpectateEntity};
use papokin_util::GameMode;

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
        // 目标必须与旁观者同世界：跨维度目标没有对应的跨维度传送
        // 实现，若放行会把旁观者拉到当前维度的同坐标处（可能嵌入
        // 方块），且相机指向异维度实体 id 会让客户端视角错乱。
        // 原版通过 changeDimension 支持跨维度旁观，在补齐该实现
        // 之前一律拒绝
        let target = world
            .get_entity_by_uuid(packet.target)
            .map(|entity| {
                let entity = entity.get_entity();
                (
                    entity.pos.load(),
                    entity.yaw.load(),
                    entity.pitch.load(),
                    entity.entity_id,
                )
            })
            .or_else(|| {
                server
                    .get_player_by_uuid(packet.target)
                    .filter(|target_player| Arc::ptr_eq(&target_player.world(), &world))
                    .map(|target_player| {
                        let entity = &target_player.living_entity.entity;
                        (
                            entity.pos.load(),
                            entity.yaw.load(),
                            entity.pitch.load(),
                            entity.entity_id,
                        )
                    })
            });
        let Some((target_pos, target_yaw, target_pitch, target_id)) = target else {
            return;
        };

        if !Self::fire_start_spectating(player, server, target_id) {
            return;
        }

        player.camera_target_id.store(Some(target_id));
        player.try_send_client_packet(&CSetCamera::new(target_id.into()));

        player.request_teleport(target_pos, target_yaw, target_pitch);
    }

    /// 触发 `PlayerStartSpectatingEntityEvent`；若事件被取消则返回 `false`
    /// 事件已被取消（相机不得移动）。
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
