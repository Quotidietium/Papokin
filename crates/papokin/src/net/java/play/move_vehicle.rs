#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_move_vehicle(&self, player: &Arc<Player>, packet: &SMoveVehicle) {
        // 本刻收到了移动数据包 — 用于 SClientTickEnd 归零跟踪。
        self.received_movement_this_tick
            .store(true, Ordering::Relaxed);
        let entity = player.get_entity();
        // 仅在确有载具时处理（原版行为）；否则改过包的客户端可以
        // 借此包直接移动无载具的自身
        let vehicle = entity
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(vehicle) = vehicle else {
            return;
        };
        // 原版校验：坐标或视角非有限直接断开，防止 NaN 持续污染实体状态
        if !packet.x.is_finite()
            || !packet.y.is_finite()
            || !packet.z.is_finite()
            || !packet.yaw.is_finite()
            || !packet.pitch.is_finite()
        {
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_INVALID_VEHICLE_MOVEMENT,
                [],
            ));
            return;
        }
        let last_pos = entity.pos.load();
        let pos = Vector3::new(packet.x, packet.y, packet.z);
        // 原版 moved too quickly（载具）：单包平方位移超过 100 驳回并拉回
        if last_pos.squared_distance_to_vec(&pos) > 100.0 {
            warn!(
                "玩家 {} 的载具移动过快（单包 {} 格），已驳回并拉回",
                player.gameprofile.name,
                last_pos.squared_distance_to_vec(&pos).sqrt()
            );
            self.force_tp(player, last_pos);
            return;
        }
        let vehicle_entity = vehicle.get_entity();
        vehicle_entity.set_pos(pos);
        vehicle_entity.set_rotation(packet.yaw, packet.pitch);
        entity.set_pos(pos);
        let distance = last_pos.squared_distance_to_vec(&pos).sqrt();
        let cm = (distance * 100.0).round() as i32;
        if cm > 0 {
            let stat = player.get_movement_statistic();
            player.increment_stat(
                papokin_data::statistic::StatisticCategory::Custom,
                stat as i32,
                cm,
            );
        }
        chunker::update_position(player);
    }
}
