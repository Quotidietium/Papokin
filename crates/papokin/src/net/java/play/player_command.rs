#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_player_command(
        &self,
        player: &Arc<Player>,
        command: &SPlayerCommand,
        server: &Arc<Server>,
    ) {
        if command.entity_id != player.entity_id().into() {
            return;
        }
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();

        let entity = &player.get_entity();
        match command.action {
            Action::StartSprinting => {
                if !entity.is_sprinting() {
                    send_cancellable_blocking! {{
                        server;
                        PlayerToggleSprintEvent::new(player.clone(), true);
                        'after: {
                            player.set_sprinting(event.is_sprinting);
                            player.update_player_pose();
                        }
                    }}
                }
            }
            Action::StopSprinting => {
                if entity.is_sprinting() {
                    send_cancellable_blocking! {{
                        server;
                        PlayerToggleSprintEvent::new(player.clone(), false);
                        'after: {
                            player.set_sprinting(event.is_sprinting);
                            player.update_player_pose();
                        }
                    }}
                }
            }
            Action::LeaveBed => player.wake_up(),

            Action::StartHorseJump => Self::handle_horse_jump(player, true),
            Action::StopHorseJump => Self::handle_horse_jump(player, false),
            Action::OpenVehicleInventory => Self::handle_open_vehicle_inventory(player),
            Action::StartFlyingElytra => {
                let fall_flying = entity.check_fall_flying();
                if entity.is_fall_flying() != fall_flying {
                    let mut event = crate::plugin::api::events::entity::entity_toggle_glide::EntityToggleGlideEvent::new(
                        entity.entity_id,
                        fall_flying,
                    );
                    server.plugin_manager.fire_blocking(server, &mut event);
                    if !event.cancelled {
                        entity.set_fall_flying(event.is_gliding);
                    }
                }
            }
            // <= 1.21.5
            Action::StartSneaking | Action::StopSneaking => {
                self.handle_player_input(
                    player,
                    &SPlayerInput {
                        input: SPlayerInput::SNEAK,
                    },
                    server,
                );
            }
        }
    }

    /// 马系骑手空格蓄力跳（原版 `PlayerRideableJumping`）：
    /// `StartHorseJump` 记录蓄力起点，`StopHorseJump` 按经过的
    /// 刻数换算跳跃力（每刻 +10、上限 100，即约 0.5 秒充满）并
    /// 让载具起跳。载具取自服务端权威的 `vehicle` 字段（客户端
    /// 不可指定），且仅马系类型响应——猪/炽足兽/骆驼无蓄力跳。
    fn handle_horse_jump(player: &Arc<Player>, start: bool) {
        let vehicle = player
            .get_entity()
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(vehicle) = vehicle else {
            return;
        };

        let entity_type = vehicle.get_entity().entity_type;
        // 原版 PlayerRideableJumping 集合：仅马系可蓄力跳
        // （猪/炽足兽/骆驼的方向键驱动不走跳跃蓄力）
        let is_rideable_jumping = [
            &papokin_data::entity::EntityType::HORSE,
            &papokin_data::entity::EntityType::DONKEY,
            &papokin_data::entity::EntityType::MULE,
            &papokin_data::entity::EntityType::SKELETON_HORSE,
            &papokin_data::entity::EntityType::ZOMBIE_HORSE,
        ]
        .contains(&entity_type);
        if !is_rideable_jumping {
            return;
        }
        let Some(mob) = vehicle.get_mob() else {
            return;
        };

        if start {
            mob.get_mob_entity()
                .jump_charge_start
                .store(Some(std::time::Instant::now()));
            return;
        }

        // 消费蓄力起点：未开始蓄力（None）时直接忽略
        let Some(started) = mob.get_mob_entity().jump_charge_start.swap(None) else {
            return;
        };
        let elapsed_ticks = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX) / 50;
        let charge = (elapsed_ticks * 10).clamp(1, 100);
        // 原版仅地面载具起跳；空中忽略（防二段跳）
        if !vehicle.get_entity().on_ground.load(Ordering::Relaxed) {
            return;
        }
        if let Some(living) = vehicle.get_living_entity() {
            living.jump_with_strength(f64::from(charge) / 100.0);
        }
    }

    /// 骑手请求打开坐骑物品栏（原版 `HasCustomInventoryScreen`，
    /// 客户端骑乘中按 E）。载具取自服务端权威的 `vehicle` 字段，
    /// 界面构造与各实体右键打开路径同源。
    fn handle_open_vehicle_inventory(player: &Arc<Player>) {
        let vehicle = player
            .get_entity()
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(vehicle) = vehicle else {
            return;
        };
        if let Some(mob) = vehicle.get_mob() {
            mob.open_rider_inventory(player);
        }
    }
}
