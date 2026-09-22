use super::EnderDragonPhase;
use crate::entity::{
    Entity, area_effect_cloud::AreaEffectCloudEntity, boss::ender_dragon::EnderDragonEntity,
};
use papokin_data::entity::EntityType;
use papokin_util::math::vector3::Vector3;

pub struct SitBreathingPhase;

impl super::Phase for SitBreathingPhase {
    fn get_type(&self) -> EnderDragonPhase {
        EnderDragonPhase::SitBreathing
    }

    fn begin(&self, dragon: &EnderDragonEntity) {
        *dragon
            .target_location
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    fn tick(&self, dragon: &EnderDragonEntity) {
        let mut timer = dragon
            .breathing_timer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *timer += 1;

        if *timer > 100 {
            *timer = 0;
            drop(timer);
            dragon.set_phase(EnderDragonPhase::SitAttacking);
            return;
        }
        drop(timer);

        let timer_val = *dragon
            .breathing_timer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timer_val == 1 {
            let entity = &dragon.mob_entity.living_entity.entity;
            let pos = entity.pos.load();
            let yaw = entity.yaw.load().to_radians() as f64;
            let world = entity.world.load();

            // 在末影龙头部位置生成滞留云
            let offset = Vector3::new(-yaw.sin() * 2.0, 0.5, yaw.cos() * 2.0);
            let cloud_pos = pos.add(&offset);

            let cloud_entity =
                Entity::new(world.clone(), cloud_pos, &EntityType::AREA_EFFECT_CLOUD);
            let cloud = AreaEffectCloudEntity::create(
                cloud_entity,
                papokin_data::item_stack::ItemStack::new(
                    0,
                    &papokin_data::item::Item::DRAGON_BREATH,
                ),
                vec![(
                    &papokin_data::effect::StatusEffect::INSTANT_DAMAGE,
                    1,
                    0,
                    false,
                    true,
                    true,
                )],
                600,  // 时长（duration）
                3.0,  // 半径
                20,   // 重新应用延迟
                20,   // 等待时间
                0.5,  // 使用时的半径
                -100, // 使用时长
            );

            // 末影龙的龙息攻击可以被插件取消。
            let mut event =
                crate::plugin::api::events::entity::ender_dragon_flame::EnderDragonFlameEvent::new(
                    entity.entity_id,
                    cloud.get_entity().entity_id,
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return;
            }

            world.spawn_entity(cloud);
        }
    }
}
