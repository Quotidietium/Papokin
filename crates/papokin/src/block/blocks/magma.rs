use std::sync::atomic::Ordering;

use papokin_data::{
    Enchantment, damage::DamageType, data_component_impl::EquipmentSlot, effect::StatusEffect,
};
use papokin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, OnEntityStepArgs};

#[pumpkin_block("minecraft:magma_block")]
pub struct MagmaBlock;

impl BlockBehaviour for MagmaBlock {
    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        {
            // 只有生物实体会受到伤害
            let Some(living_entity) = args.entity.get_living_entity() else {
                return;
            };

            let ent = args.entity.get_entity();

            // 潜行时不造成伤害
            if ent.is_sneaking() {
                return;
            }

            // 免疫火焰的实体不会受到伤害
            if ent.entity_type.fire_immune || ent.fire_immune.load(Ordering::Relaxed) {
                return;
            }

            let has_frost_walker = {
                let equipment = living_entity
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equipment
                    .equipment
                    .get(&EquipmentSlot::FEET)
                    .is_some_and(|boots| {
                        boots.get_enchantment_level(&Enchantment::FROST_WALKER) != 0
                    })
            };
            if has_frost_walker {
                return;
            }

            if living_entity
                .get_effect(&StatusEffect::FIRE_RESISTANCE)
                .is_some()
            {
                return;
            }

            // 施加伤害
            args.entity.damage(args.entity, 1.0, DamageType::HOT_FLOOR);
        }
    }
}
