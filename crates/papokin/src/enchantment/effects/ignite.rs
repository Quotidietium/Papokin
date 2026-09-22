use std::sync::Arc;

use papokin_data::enchantment::LevelBasedValue;
use papokin_util::math::vector3::Vector3;

use super::EnchantmentEntityEffectExt;
use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase};
use crate::world::World;

/// 点燃实体一段时间的附魔实体效果，时长由等级计算得出。
#[derive(Clone, Debug, PartialEq)]
pub struct Ignite {
    pub duration: LevelBasedValue,
}

impl Ignite {
    #[must_use]
    pub const fn new(duration: LevelBasedValue) -> Self {
        Self { duration }
    }

    /// 根据给定附魔等级对实体施加点燃效果。
    /// 当 `combuster` 已知时，会触发 `EntityCombustByEntityEvent`，以便
    /// 插件可以取消或缩短燃烧。
    pub fn apply_to_entity(&self, level: i32, entity: &Entity, combuster: Option<i32>) {
        let seconds = self.duration.calculate(level);
        if let Some(combuster_id) = combuster {
            let world = entity.world.load();
            let mut combust_event =
                crate::plugin::api::events::entity::entity_combust_by_entity::EntityCombustByEntityEvent::new(
                    entity.entity_id,
                    combuster_id,
                    seconds,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut combust_event);
            }
            if combust_event.cancelled {
                return;
            }
            entity.set_on_fire_for(combust_event.duration);
        } else {
            entity.set_on_fire_for(seconds);
        }
        entity.set_on_fire(true);
    }
}

impl EnchantmentEntityEffectExt for Ignite {
    fn apply(
        &self,
        _world: &Arc<World>,
        enchantment_level: i32,
        owner: Option<&Arc<Player>>,
        entity: Option<&Entity>,
        _position: Vector3<f64>,
    ) {
        if let Some(entity) = entity {
            self.apply_to_entity(
                enchantment_level,
                entity,
                owner.map(|player| player.get_entity().entity_id),
            );
        }
    }
}
