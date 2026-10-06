use crate::{
    entity::{Entity, EntityBase, projectile::ThrownItemEntity},
    server::Server,
    world::World,
};
use papokin_data::data_component_impl::FireworksImpl;
use papokin_data::entity::EntityStatus;
use papokin_data::item_stack::ItemStack;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::codec::optional_int::OptionalInt;
use papokin_util::{
    math::vector3::Vector3,
    random::{RandomGenerator, RandomImpl, get_seed, xoroshiro128::Xoroshiro},
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const GRAVITY: f64 = 0.0;

pub struct FireworkRocketEntity {
    entity: ThrownItemEntity,
    life: AtomicU32,
    life_time: AtomicU32,
}

impl FireworkRocketEntity {
    pub fn new(entity: Entity) -> Self {
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));

        entity.set_velocity(Vector3::new(
            random.next_triangular(0.0, 0.002_297),
            0.05,
            random.next_triangular(0.0, 0.002_297),
        ));
        Self {
            entity: ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: AtomicBool::new(false),
                gravity: GRAVITY,
            },
            life: 0.into(),
            life_time: (10 + random.next_bounded_i32(6) as u32 + random.next_bounded_i32(7) as u32)
                .into(),
        }
    }

    pub fn new_shot(entity: Entity, shooter: &Entity) -> Self {
        let rocket = Self::new(entity);
        let rocket = Self {
            entity: ThrownItemEntity {
                owner_id: Some(shooter.entity_id),
                ..rocket.entity
            },
            ..rocket
        };
        rocket.get_entity().set_pos(shooter.pos.load());
        rocket.get_entity().set_velocity(shooter.velocity.load());
        rocket.get_entity().set_synced_data(
            papokin_data::tracked_data::firework_rocket::ATTACHED_TO_TARGET,
            OptionalInt(Some(shooter.entity_id)),
        );

        rocket
    }

    /// 同步烟花物品并按飞行时长设置寿命。
    pub fn set_item_stack(&self, stack: ItemStack) {
        let flight = stack
            .get_data_component::<FireworksImpl>()
            .map_or(0, |data| data.flight_duration);
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));
        self.life_time.store(
            10 * (1 + flight.clamp(0, 255) as u32)
                + random.next_bounded_i32(6) as u32
                + random.next_bounded_i32(7) as u32,
            Ordering::Relaxed,
        );
        self.get_entity().set_synced_data(
            papokin_data::tracked_data::firework_rocket::ID_FIREWORKS_ITEM,
            ItemStackSerializer::from(stack),
        );
    }

    pub fn explode_and_remove(&self, world: &World) {
        let entity = self.get_entity();
        if let Some(server) = world.server.upgrade() {
            let mut event =
                crate::plugin::api::events::entity::firework_explode::FireworkExplodeEvent {
                    entity_id: entity.entity_id,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        world.send_entity_status(entity, EntityStatus::FireworksExplode);

        entity.remove();
    }
}

impl EntityBase for FireworkRocketEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.entity.owner_id
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = self.get_entity();
        let world = entity.world.load();
        if self.entity.owner_id.is_some() {
            // 绑定烟花仅跟随目标，不执行普通投射物的惯性、移动与碰撞。
            entity.update_last_pos();
        } else {
            self.entity.process_tick(caller);
            if entity.removed.load(Ordering::Relaxed) {
                return;
            }
        }
        let mut velocity = entity.velocity.load();

        if let Some(shooter_id) = self.entity.owner_id {
            if let Some(shooter) = world.get_entity_by_id(shooter_id) {
                let shooter = shooter.get_entity();

                if shooter.is_fall_flying() {
                    let mut boost_cancelled = false;
                    if let Some(player) = world.get_player_by_id(shooter_id)
                        && let Some(server) = world.server.upgrade()
                    {
                        let mut event = crate::plugin::api::events::player::player_elytra_boost::PlayerElytraBoostEvent {
                            player,
                            firework_id: entity.entity_id,
                            cancelled: false,
                        };
                        server.plugin_manager.fire_blocking(&server, &mut event);
                        if event.cancelled {
                            boost_cancelled = true;
                        }
                    }
                    if !boost_cancelled {
                        let rotation = shooter.rotation().to_f64();
                        let shooter_vel = shooter.velocity.load();

                        let new_shooter_vel =
                            shooter_vel + (rotation * 0.1 + (rotation * 1.5 - shooter_vel) * 0.5);

                        shooter.set_velocity(new_shooter_vel);
                    }
                }
                // 停止滑翔或插件取消加速时仍保持绑定，直到寿命结束。
                entity.set_pos(shooter.pos.load());
                entity.set_velocity(shooter.velocity.load());
            } else {
                entity.remove();
                return;
            }
        } else {
            velocity.x *= 1.15;
            velocity.z *= 1.15;
            velocity.y += 0.04;
            entity.set_velocity(velocity);
        }

        let current_life = self.life.fetch_add(1, Ordering::Relaxed);
        if current_life > self.life_time.load(Ordering::Relaxed) {
            self.explode_and_remove(&world);
        }
    }

    fn get_entity(&self) -> &crate::entity::Entity {
        &self.entity.entity
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn on_hit(&self, _hit: crate::entity::projectile::ProjectileHit) {
        let world = self.get_entity().world.load();
        self.explode_and_remove(&world);
    }
}

#[cfg(test)]
mod tests {
    use papokin_data::tracked_data::firework_rocket;
    use papokin_util::version::JavaMinecraftVersion;

    #[test]
    fn rocket_attachment_and_item_exist_in_supported_protocols() {
        for version in [
            JavaMinecraftVersion::V_1_21,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_3,
        ] {
            assert_eq!(firework_rocket::ATTACHED_TO_TARGET.get(&version), 9);
            assert_eq!(firework_rocket::ID_FIREWORKS_ITEM.get(&version), 8);
            assert_eq!(firework_rocket::SHOT_AT_ANGLE.get(&version), 10);
        }
    }
}
