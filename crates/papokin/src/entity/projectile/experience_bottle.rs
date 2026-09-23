use std::sync::atomic::AtomicBool;

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::projectile::{ProjectileHit, ThrownItemEntity};
use crate::entity::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use papokin_data::entity::EntityStatus;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use rand::RngExt;

const GRAVITY: f64 = 0.03;

pub struct ExperienceBottleEntity {
    pub thrown: ThrownItemEntity,
}

impl ExperienceBottleEntity {
    pub fn new(entity: Entity) -> Self {
        // 投掷的经验瓶默认速度略微向上
        entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));
        let thrown = ThrownItemEntity {
            entity,
            owner_id: None,
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            gravity: GRAVITY,
        };
        Self { thrown }
    }

    pub fn new_shot(entity: Entity, shooter: &Entity) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        thrown.entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));
        Self { thrown }
    }
}

impl EntityBase for ExperienceBottleEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.thrown.owner_id
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.thrown.process_tick(caller);
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn on_hit(&self, _hit: ProjectileHit) {
        let world = self.get_entity().world.load();
        let pos = self.get_entity().pos.load();
        // 原版数量：3 + rand(5) + rand(5)，即 3..=13 点经验
        let mut rng = rand::rng();
        let amount = 3 + rng.random_range(0..5) + rng.random_range(0..5);

        // 经验瓶破裂事件，取消则瓶子不破裂也不释放经验
        let mut event = crate::plugin::api::events::entity::exp_bottle::ExpBottleEvent::new(
            self.get_entity().entity_id,
            amount as i32,
            BlockPos::new(
                pos.x.floor() as i32,
                pos.y.floor() as i32,
                pos.z.floor() as i32,
            ),
            true,
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }

        // 播放玻璃瓶碎裂粒子；经验钳到合理上限，
        // 防止插件写入异常大值时海量生成经验球阻塞 tick
        world.send_entity_status(self.get_entity(), EntityStatus::Death);
        ExperienceOrbEntity::spawn(&world, pos, event.experience.clamp(0, 1000) as u32);
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
