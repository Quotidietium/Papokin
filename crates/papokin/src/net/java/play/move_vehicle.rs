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
        let vehicle_entity = vehicle.get_entity();
        // 原版仅“控制乘客”（乘客表首位）驱动载具移动；其余乘客的
        // 载具移动包一律忽略，防止后座客户端抢占载具位置。
        let is_controller = vehicle_entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .is_some_and(|passenger| passenger.get_entity().entity_id == entity.entity_id);
        if !is_controller {
            return;
        }
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
        // 与玩家移动包相同的三项守卫：等待传送确认期间忽略（否则传送后
        // 载具仍会被留在客户端声称的位置）、插件锁定移动时拉回、坐标
        // clamp 到世界边界内。
        if player
            .awaiting_teleport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            return;
        }
        if player.is_movement_locked.load(Ordering::Relaxed) {
            self.force_tp(player, player.get_entity().pos.load());
            return;
        }
        let pos = Vector3::new(
            Self::clamp_horizontal(packet.x),
            Self::clamp_vertical(packet.y),
            Self::clamp_horizontal(packet.z),
        );
        let last_pos = entity.pos.load();
        // 原版 moved too quickly（载具）：阈值随本刻移动包序缩放
        //（与玩家移动包一致的按刻累积语义）
        let packet_index = self
            .movement_packets_this_tick
            .fetch_add(1, Ordering::Relaxed);
        let allowed_delta_squared = 100.0 * f64::from(packet_index.max(1));
        if last_pos.squared_distance_to_vec(&pos) > allowed_delta_squared {
            warn!(
                "玩家 {} 的载具移动过快（单包 {} 格），已驳回并拉回",
                player.gameprofile.name,
                last_pos.squared_distance_to_vec(&pos).sqrt()
            );
            self.force_tp(player, last_pos);
            return;
        }
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
