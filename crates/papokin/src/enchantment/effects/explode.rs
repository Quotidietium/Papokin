use papokin_data::damage::DamageType;
use papokin_data::enchantment::LevelBasedValue;
use papokin_data::particle::Particle;
use papokin_data::sound::Sound;
use papokin_util::math::vector3::Vector3;

use crate::world::ExplosionInteraction;

/// 在偏移位置产生爆炸的附魔实体效果。
#[derive(Clone, Debug, PartialEq)]
pub struct ExplodeEffect {
    pub attribute_to_user: bool,
    pub damage_type: Option<&'static DamageType>,
    pub knockback_multiplier: Option<LevelBasedValue>,
    pub immune_blocks: Option<&'static str>,
    pub offset: Vector3<f64>,
    pub radius: LevelBasedValue,
    pub create_fire: bool,
    pub block_interaction: ExplosionInteraction,
    pub small_particle: Option<Particle>,
    pub large_particle: Option<Particle>,
    pub sound: Option<Sound>,
}

impl ExplodeEffect {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        attribute_to_user: bool,
        damage_type: Option<&'static DamageType>,
        knockback_multiplier: Option<LevelBasedValue>,
        immune_blocks: Option<&'static str>,
        offset: Vector3<f64>,
        radius: LevelBasedValue,
        create_fire: bool,
        block_interaction: ExplosionInteraction,
        small_particle: Option<Particle>,
        large_particle: Option<Particle>,
        sound: Option<Sound>,
    ) -> Self {
        Self {
            attribute_to_user,
            damage_type,
            knockback_multiplier,
            immune_blocks,
            offset,
            radius,
            create_fire,
            block_interaction,
            small_particle,
            large_particle,
            sound,
        }
    }

    #[must_use]
    pub fn calculate_radius(&self, level: i32) -> f32 {
        self.radius.calculate(level).max(0.0)
    }

    #[must_use]
    pub fn calculate_knockback(&self, level: i32) -> Option<f32> {
        self.knockback_multiplier
            .as_ref()
            .map(|kb| kb.calculate(level))
    }
}

impl super::EnchantmentEntityEffectExt for ExplodeEffect {
    fn apply(
        &self,
        world: &std::sync::Arc<crate::world::World>,
        enchantment_level: i32,
        _owner: Option<&std::sync::Arc<crate::entity::player::Player>>,
        _entity: Option<&crate::entity::Entity>,
        position: Vector3<f64>,
    ) {
        let r = self.calculate_radius(enchantment_level);
        let center = position + self.offset;
        world.explode(
            center,
            r,
            if self.create_fire {
                self.block_interaction
            } else {
                ExplosionInteraction::None
            },
        );
    }
}
