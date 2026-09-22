#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_rotation(&self, player: &Player, rotation: &SPlayerRotation) {
        if !player.has_client_loaded() {
            return;
        }
        // 本刻收到了移动数据包 — 用于 SClientTickEnd 归零跟踪。
        self.received_movement_this_tick
            .store(true, Ordering::Relaxed);
        if !rotation.yaw.is_finite() || !rotation.pitch.is_finite() {
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_INVALID_PLAYER_MOVEMENT,
                [],
            ));
            return;
        }
        let entity = &player.get_entity();
        entity.on_ground.store(rotation.ground, Ordering::Relaxed);
        entity.set_rotation(
            wrap_degrees(rotation.yaw) % 360.0,
            wrap_degrees(rotation.pitch),
        );
        // 将新位置发送给所有其他玩家。
        let entity_id = entity.entity_id;
        let yaw = (entity.yaw.load() * 256.0 / 360.0).rem_euclid(256.0);
        let pitch = (entity.pitch.load() * 256.0 / 360.0).rem_euclid(256.0);
        // let head_yaw = modulus(entity.head_yaw * 256.0 / 360.0, 256.0);

        let world = entity.world.load_full();
        let je_packet =
            CUpdateEntityRot::new(entity_id.into(), yaw as u8, pitch as u8, rotation.ground);

        world.broadcast_packet_except(&[player.gameprofile.id], &je_packet);

        let je_packet = CHeadRot::new(entity_id.into(), yaw as u8);
        world.broadcast_packet_except(&[player.gameprofile.id], &je_packet);
    }
}
