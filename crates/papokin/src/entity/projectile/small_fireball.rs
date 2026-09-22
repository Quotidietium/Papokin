use std::sync::atomic::AtomicBool;

use papokin_util::math::vector3::Vector3;

use crate::{
    entity::{
        Entity, EntityBase,
        projectile::{ProjectileHit, ThrownItemEntity, fireball::INITIAL_ACCELERATION_POWER},
    },
    server::Server,
};

const GRAVITY: f64 = 0.0;

pub struct SmallFireballEntity {
    pub thrown: ThrownItemEntity,
}

impl SmallFireballEntity {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        let thrown = ThrownItemEntity {
            entity,
            owner_id: None,
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            gravity: GRAVITY,
        };

        Self { thrown }
    }

    #[must_use]
    pub fn new_shot(entity: Entity, shooter: &Entity, direction: Vector3<f64>) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        let accel = INITIAL_ACCELERATION_POWER;
        thrown
            .entity
            .velocity
            .store(direction.normalize().multiply(accel, accel, accel));
        Self { thrown }
    }
}

impl EntityBase for SmallFireballEntity {
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

    fn on_hit(&self, hit: ProjectileHit) {
        match hit {
            ProjectileHit::Entity { ref entity, .. } => {
                let mut combust_event = crate::plugin::api::events::entity::entity_combust_by_entity::EntityCombustByEntityEvent::new(
                    entity.get_entity().entity_id,
                    self.get_entity().entity_id,
                    5.0,
                );
                if let Some(server) = self.get_entity().world.load().server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut combust_event);
                }
                if !combust_event.cancelled {
                    entity.get_entity().set_on_fire_for(5.0);
                }
                let _ = entity.damage(
                    entity.as_ref(),
                    5.0,
                    papokin_data::damage::DamageType::FIREBALL,
                );
            }
            ProjectileHit::Block { pos, face, .. } => {
                // 尝试放置火
                let block_to_place = match face {
                    papokin_data::BlockDirection::Up => pos.up(),
                    papokin_data::BlockDirection::Down => pos.down(),
                    papokin_data::BlockDirection::North => pos.north(),
                    papokin_data::BlockDirection::South => pos.south(),
                    papokin_data::BlockDirection::West => pos.west(),
                    papokin_data::BlockDirection::East => pos.east(),
                };
                let world = self.get_entity().world.load();
                let fire_state = papokin_data::Block::FIRE.default_state.id;
                world.set_block_state(
                    &block_to_place,
                    fire_state,
                    papokin_world::world::BlockFlags::NOTIFY_ALL,
                );
            }
        }
    }
}
