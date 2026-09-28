use core::f32;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use papokin_data::entity::EntityType;
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::vector3::Vector3;

use crate::{server::Server, world::World};

use super::{Entity, EntityBase, living::LivingEntity, player::Player};

pub struct ExperienceOrbEntity {
    entity: Entity,
    /// 经验值。原子存取以支持 `read_custom_nbt`（&self）重载。
    amount: AtomicU32,
    orb_age: AtomicU32,
}

impl ExperienceOrbEntity {
    pub fn new(entity: Entity, amount: u32) -> Self {
        entity.yaw.store(rand::random::<f32>() * 360.0);
        Self {
            entity,
            amount: AtomicU32::new(amount),
            orb_age: AtomicU32::new(0),
        }
    }

    pub fn spawn(world: &Arc<World>, position: Vector3<f64>, amount: u32) {
        let mut amount = amount;
        while amount > 0 {
            let i = Self::round_to_orb_size(amount);
            amount -= i;
            let entity = Entity::new(world.clone(), position, &EntityType::EXPERIENCE_ORB);
            let orb = Arc::new(Self::new(entity, i));
            world.spawn_entity(orb);
        }
    }

    const fn round_to_orb_size(value: u32) -> u32 {
        if value >= 2477 {
            2477
        } else if value >= 1237 {
            1237
        } else if value >= 617 {
            617
        } else if value >= 307 {
            307
        } else if value >= 149 {
            149
        } else if value >= 73 {
            73
        } else if value >= 37 {
            37
        } else if value >= 17 {
            17
        } else if value >= 7 {
            7
        } else if value >= 3 {
            3
        } else {
            1
        }
    }
}

impl EntityBase for ExperienceOrbEntity {
    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        let entity = &self.entity;
        entity.tick(caller, server);
        let bounding_box = entity.bounding_box.load();

        let original_velo = entity.velocity.load();

        let mut velo = original_velo;

        let no_physics = !self
            .entity
            .world
            .load()
            .is_space_empty(bounding_box.expand(-1.0e-7, -1.0e-7, -1.0e-7));
        self.entity.no_physics.store(no_physics, Ordering::Relaxed);
        // TODO: isSubmergedIn
        if !no_physics {
            velo.y -= self.get_gravity();
        }

        entity.velocity.store(velo);

        entity.move_entity(caller, velo);

        entity.tick_block_collisions(caller);

        let age = self.orb_age.fetch_add(1, Ordering::Relaxed);
        if age >= 6000 {
            entity.remove();
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        // 年龄饱和：>=6000 的球下一刻即自清，无需也不应写入更大值
        nbt.put_short(
            "Age",
            self.orb_age.load(Ordering::Relaxed).min(i16::MAX as u32) as i16,
        );
        // round_to_orb_size 上限 2477，理论上不会超 short；饱和纯防御
        nbt.put_short(
            "Value",
            self.amount.load(Ordering::Relaxed).min(i16::MAX as u32) as i16,
        );
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        // 负值（损坏存档）归零；年龄 >=6000 由 tick 正常自清
        self.orb_age.store(
            nbt.get_short("Age").unwrap_or(0).max(0) as u32,
            Ordering::Relaxed,
        );
        // 零值经验球是退化数据（拾取不给经验却占实体槽位），钳到 1
        self.amount.store(
            nbt.get_short("Value").unwrap_or(1).max(1) as u32,
            Ordering::Relaxed,
        );
    }

    fn on_player_collision(&self, player: &Arc<Player>) {
        if player.living_entity.health.load() > 0.0 {
            let can_pickup = if let Ok(mut delay) = player.experience_pick_up_delay.try_lock()
                && *delay == 0
            {
                let mut cooldown_event = crate::plugin::api::events::player::player_exp_cooldown_change::PlayerExpCooldownChangeEvent {
                    player: player.clone(),
                    new_cooldown: 2,
                    cancelled: false,
                };
                let world = self.entity.world.load();
                if let Some(server) = world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut cooldown_event);
                }
                if cooldown_event.cancelled {
                    false
                } else {
                    *delay = cooldown_event.new_cooldown.max(0) as u32;
                    true
                }
            } else {
                false
            };
            if can_pickup {
                // 拾取钩子：取消会让经验球留在世界中；
                // 数值可被处理器调整。
                let orb_id = self.entity.entity_id;
                let amount = self.amount.load(Ordering::Relaxed) as i32;
                let mut pickup_event = crate::plugin::api::events::player::player_pickup_experience::PlayerPickupExperienceEvent::new(
                    player.clone(),
                    orb_id,
                    amount,
                );
                let world = self.entity.world.load();
                if let Some(server) = world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut pickup_event);
                }
                if pickup_event.cancelled {
                    return;
                }
                let amount = pickup_event.amount.max(0);

                player.living_entity.pickup(&self.entity, 1);
                self.entity.remove();
                let remaining = player.apply_mending_from_xp(amount);
                if remaining > 0 {
                    player.add_experience_points(remaining);
                }
            }
        }
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }
    fn get_gravity(&self) -> f64 {
        0.03
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
