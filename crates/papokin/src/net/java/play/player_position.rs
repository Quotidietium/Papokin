#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    const fn clamp_horizontal(pos: f64) -> f64 {
        pos.clamp(-3.0E7, 3.0E7)
    }

    const fn clamp_vertical(pos: f64) -> f64 {
        pos.clamp(-2.0E7, 2.0E7)
    }

    /// 返回是否需要同步位置
    fn sync_position(
        player: &Arc<Player>,
        world: &World,
        pos: Vector3<f64>,
        last_pos: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) -> bool {
        let delta = Vector3::new(pos.x - last_pos.x, pos.y - last_pos.y, pos.z - last_pos.z);
        let entity_id = player.entity_id();

        // 位移超过 8 格时传送（-8..=7.999755859375）
        if delta.length_squared() < 64.0 {
            return false;
        }
        // 将位置同步给所有其他玩家。
        world.broadcast_packet_except(
            &[player.gameprofile.id],
            &CEntityPositionSync::new(
                entity_id.into(),
                pos,
                Vector3::new(0.0, 0.0, 0.0),
                yaw,
                pitch,
                on_ground,
            ),
        );
        true
    }

    #[expect(clippy::too_many_lines)]
    pub fn handle_position(
        &self,
        player: &Arc<Player>,
        server: &Arc<Server>,
        packet: &SPlayerPosition,
    ) {
        if !player.has_client_loaded() {
            return;
        }
        // 本刻收到了移动数据包 — 用于 SClientTickEnd 归零跟踪。
        self.received_movement_this_tick
            .store(true, Ordering::Relaxed);
        if player.get_entity().has_vehicle() {
            return;
        }
        // 在等待传送确认时忽略移动数据包（原版行为）
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
        // y = 脚部 Y
        let position = packet.position;
        if position.x.is_nan() || position.y.is_nan() || position.z.is_nan() {
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_INVALID_PLAYER_MOVEMENT,
                [],
            ));
            return;
        }
        let position = Vector3::new(
            Self::clamp_horizontal(position.x),
            Self::clamp_vertical(position.y),
            Self::clamp_horizontal(position.z),
        );

        // 原版 moved too quickly：单包平方位移超过阈值（飞行 300，普通
        // 100）时驳回并拉回，否则改过包的客户端可以任意距离瞬移
        let previous_pos = player.get_entity().pos.load();
        let flying = player
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flying;
        let max_delta_squared = if flying { 300.0 } else { 100.0 };
        if previous_pos.squared_distance_to_vec(&position) > max_delta_squared {
            warn!(
                "玩家 {} 移动过快（单包 {} 格），已驳回并拉回",
                player.gameprofile.name,
                previous_pos.squared_distance_to_vec(&position).sqrt()
            );
            self.force_tp(player, previous_pos);
            return;
        }

        send_cancellable_blocking! {{
            server;
            PlayerMoveEvent {
                player: player.clone(),
                from: player.get_entity().pos.load(),
                to: position,
                cancelled: false,
            };

            'after: {
                let pos = event.to;
                let entity = &player.get_entity();
                let last_pos = entity.pos.load();
                player.get_entity().set_pos(pos);

                let distance = last_pos.squared_distance_to_vec(&pos).sqrt();
                let cm = (distance * 100.0) as i32;
                if cm > 0 {
                    let stat = player.get_movement_statistic();
                    player.increment_stat(StatisticCategory::Custom, stat as i32, cm);
                }

                let height_difference = pos.y - last_pos.y;
                if entity.on_ground.load(Ordering::Relaxed) && packet.collision & FLAG_ON_GROUND == 0 && height_difference > 0.0 {
                    player.jump();
                }

                let new_on_ground = packet.collision & FLAG_ON_GROUND != 0;
                entity.on_ground.store(new_on_ground, Ordering::Relaxed);
                if new_on_ground && entity.is_fall_flying() {
                    entity.set_fall_flying(false);
                }
                let world = &player.world();

                // TODO: 玩家移动过快时发出警告
                if !Self::sync_position(player, world, pos, last_pos, entity.yaw.load(), entity.pitch.load(), packet.collision & FLAG_ON_GROUND != 0) {
                    // 将新位置发送给所有其他玩家。
                    world.broadcast_packet_except(
                        &[player.gameprofile.id],
                        &CUpdateEntityPos::new(
                            player.entity_id().into(),
                            Vector3::new(
                                pos.x.mul_add(4096.0, -(last_pos.x * 4096.0)) as i16,
                                pos.y.mul_add(4096.0, -(last_pos.y * 4096.0)) as i16,
                                pos.z.mul_add(4096.0, -(last_pos.z * 4096.0)) as i16,
                            ),
                            packet.collision & FLAG_ON_GROUND != 0,
                        ),
                    );
                }

                // 仅在玩家存活时才处理摔落伤害
                if !player.abilities.lock().unwrap_or_else(std::sync::PoisonError::into_inner).flying
                    && player.living_entity.health.load() > 0.0
                    && !player.living_entity.dead.load(Ordering::Relaxed)
                {
                    player.living_entity.fall(
                        player.as_ref(),
                        height_difference,
                        packet.collision & FLAG_ON_GROUND != 0,
                        player.gamemode.load() == GameMode::Creative,
                    );
                }
                chunker::update_position(player);
                let delta = Vector3::new(
                    pos.x - last_pos.x,
                    pos.y - last_pos.y,
                    pos.z - last_pos.z,
                );
                // 仅在实际发生移动时才更新空闲超时（原版阈值）
                if delta.length_squared() > 1.0E-5 {
                    player.update_last_action_time();
                    player.check_location_enchantments(pos, packet.collision & FLAG_ON_GROUND != 0);
                }
                player.progress_motion(delta);
            }

            'cancelled: {
                self.force_tp(player, player.get_entity().pos.load());
            }
        }}
    }

    #[expect(clippy::too_many_lines)]
    pub fn handle_position_rotation(
        &self,
        player: &Arc<Player>,
        server: &Arc<Server>,
        packet: &SPlayerPositionRotation,
    ) {
        if !player.has_client_loaded() {
            return;
        }
        // 本刻收到了移动数据包 — 用于 SClientTickEnd 归零跟踪。
        self.received_movement_this_tick
            .store(true, Ordering::Relaxed);
        if player.get_entity().has_vehicle() {
            return;
        }
        // 在等待传送确认时忽略移动数据包（原版行为）
        if player
            .awaiting_teleport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            return;
        }
        if player.is_movement_locked.load(Ordering::Relaxed) {
            let entity = player.get_entity();
            entity.set_rotation(packet.yaw, packet.pitch);
            self.force_tp(player, entity.pos.load());
            return;
        }
        // y = 脚部 Y
        let position = packet.position;
        if !position.x.is_finite()
            || !position.y.is_finite()
            || !position.z.is_finite()
            || !packet.yaw.is_finite()
            || !packet.pitch.is_finite()
        {
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_INVALID_PLAYER_MOVEMENT,
                [],
            ));
            return;
        }

        let position = Vector3::new(
            Self::clamp_horizontal(position.x),
            Self::clamp_vertical(position.y),
            Self::clamp_horizontal(position.z),
        );

        // 原版 moved too quickly：阈值与不带旋转的分支一致
        let previous_pos = player.get_entity().pos.load();
        let flying = player
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flying;
        let max_delta_squared = if flying { 300.0 } else { 100.0 };
        if previous_pos.squared_distance_to_vec(&position) > max_delta_squared {
            warn!(
                "玩家 {} 移动过快（单包 {} 格），已驳回并拉回",
                player.gameprofile.name,
                previous_pos.squared_distance_to_vec(&position).sqrt()
            );
            self.force_tp(player, previous_pos);
            return;
        }

        send_cancellable_blocking! {{
            server;
            PlayerMoveEvent::new(
                player.clone(),
                player.get_entity().pos.load(),
                position,
            );

            'after: {
                let pos = event.to;
                let entity = &player.get_entity();
                let last_pos = entity.pos.load();
                player.get_entity().set_pos(pos);

                let distance = last_pos.squared_distance_to_vec(&pos).sqrt();
                let cm = (distance * 100.0) as i32;
                if cm > 0 {
                    let stat = player.get_movement_statistic();
                    player.increment_stat(StatisticCategory::Custom, stat as i32, cm);
                }

                let height_difference = pos.y - last_pos.y;
                if entity.on_ground.load(Ordering::Relaxed)
                    && (packet.collision & FLAG_ON_GROUND) != 0
                    && height_difference > 0.0
                {
                    player.jump();
                }
                entity
                    .on_ground
                    .store((packet.collision & FLAG_ON_GROUND) != 0, Ordering::Relaxed);

                entity.set_rotation(wrap_degrees(packet.yaw) % 360.0, wrap_degrees(packet.pitch));

                let entity_id = entity.entity_id;

                let yaw = (entity.yaw.load() * 256.0 / 360.0).rem_euclid(256.0);
                let pitch = (entity.pitch.load() * 256.0 / 360.0).rem_euclid(256.0);
                // let head_yaw = (entity.head_yaw * 256.0 / 360.0).floor();
                let world = entity.world.load_full();

                // TODO: 玩家移动过快时发出警告
                if !Self::
                    sync_position(player, &world, pos, last_pos, yaw, pitch, (packet.collision & FLAG_ON_GROUND) != 0)
                {
                    // 将新位置发送给所有其他玩家。
                    world.broadcast_packet_except(
                        &[player.gameprofile.id],
                        &CUpdateEntityPosRot::new(
                            entity_id.into(),
                            Vector3::new(
                                pos.x.mul_add(4096.0, -(last_pos.x * 4096.0)) as i16,
                                pos.y.mul_add(4096.0, -(last_pos.y * 4096.0)) as i16,
                                pos.z.mul_add(4096.0, -(last_pos.z * 4096.0)) as i16,
                            ),
                            yaw as u8,
                            pitch as u8,
                            (packet.collision & FLAG_ON_GROUND) != 0,
                        ),
                    );
                }

                world
                    .broadcast_packet_except(
                        &[player.gameprofile.id],
                        &CHeadRot::new(entity_id.into(), yaw as u8),
                    )
                   ;
                // 仅在玩家存活时才处理摔落伤害
                if !player.abilities.lock().unwrap_or_else(std::sync::PoisonError::into_inner).flying
                    && player.living_entity.health.load() > 0.0
                    && !player.living_entity.dead.load(Ordering::Relaxed)
                {
                    player.living_entity.fall(
                        player.as_ref(),
                        height_difference,
                        (packet.collision & FLAG_ON_GROUND) != 0,
                        player.gamemode.load() == GameMode::Creative,
                    );
                }
                chunker::update_position(player);
                let delta = Vector3::new(
                    pos.x - last_pos.x,
                    pos.y - last_pos.y,
                    pos.z - last_pos.z,
                );
                // 仅在实际发生移动时才更新空闲超时（原版阈值）
                if delta.length_squared() > 1.0E-5 {
                    player.update_last_action_time();
                    player.check_location_enchantments(pos, (packet.collision & FLAG_ON_GROUND) != 0);
                }
                player.progress_motion(delta);
            }

            'cancelled: {
                self.force_tp(player, position);
            }
        }}
    }

    pub fn force_tp(&self, player: &Arc<Player>, position: Vector3<f64>) {
        let teleport_id = player.teleport_id_count.fetch_add(1, Ordering::Relaxed) + 1;
        *player
            .awaiting_teleport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((teleport_id.into(), position));
        player.try_send_client_packet(&CPlayerPosition::new(
            teleport_id.into(),
            player.get_entity().pos.load(),
            Vector3::new(0.0, 0.0, 0.0),
            player.get_entity().yaw.load(),
            player.get_entity().pitch.load(),
            Vec::new(),
        ));
    }
}
