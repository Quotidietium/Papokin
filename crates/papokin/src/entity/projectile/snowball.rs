use std::sync::atomic::AtomicBool;

use crate::entity::projectile::ProjectileHit;
use crate::{
    entity::{Entity, EntityBase, projectile::ThrownItemEntity},
    server::Server,
};
use papokin_data::damage::DamageType;
use papokin_data::entity::{EntityStatus, EntityType};
use papokin_util::math::vector3::Vector3;

const GRAVITY: f64 = 0.03;

pub struct SnowballEntity {
    pub thrown: ThrownItemEntity,
}

impl SnowballEntity {
    pub fn new(entity: Entity) -> Self {
        // 保留速度初始化
        entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));

        // 在无所有者的情况下初始化
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

impl EntityBase for SnowballEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.thrown.owner_id
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.thrown.process_tick(caller);
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn on_hit(&self, hit: crate::entity::projectile::ProjectileHit) {
        let world = self.get_entity().world.load();

        // 无论命中什么都发送粒子状态
        world.send_entity_status(self.get_entity(), EntityStatus::Death);

        // 处理特定于实体的伤害
        if let ProjectileHit::Entity { ref entity, .. } = hit {
            let is_blaze = entity.get_entity().entity_type.id == EntityType::BLAZE.id;
            let damage = if is_blaze { 3.0 } else { 0.0 }; // 只对烈焰人造成伤害

            entity.damage(entity.as_ref(), damage, DamageType::THROWN);
        }
    }
}
