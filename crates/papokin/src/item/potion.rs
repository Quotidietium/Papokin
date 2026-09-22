use crate::entity::EntityBase;
use crate::entity::living::LivingEntity;
use papokin_data::effect::StatusEffect;
use papokin_data::item_stack::ItemStack;

/// 用于从 `ItemStack` 读取药水内容并应用效果的工具。
pub struct PotionContents;

/// 应用药水效果的来源上下文（影响缩放规则）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PotionApplicationSource {
    /// 常规使用方式（饮用 / 喷溅）
    Normal,
    /// `AreaEffectCloud` 生效时（持续时间更短、瞬时强度更低）
    AreaEffectCloud,
    Arrow,
}

impl PotionApplicationSource {
    const fn instant_scale(self, scale: f32) -> f32 {
        match self {
            Self::AreaEffectCloud => scale * 0.5,
            Self::Arrow => 1.0,
            Self::Normal => scale,
        }
    }

    const fn duration_scale(self, scale: f32) -> f32 {
        match self {
            Self::AreaEffectCloud => scale * 0.25,
            Self::Arrow | Self::Normal => scale,
        }
    }
}

impl PotionContents {
    /// 从 `ItemStack` 的 `PotionContents` 数据组件读取效果。
    #[must_use]
    pub fn read_potion_effects(
        stack: &ItemStack,
    ) -> Vec<(&'static StatusEffect, i32, u8, bool, bool, bool)> {
        // 优先使用生成的药水 ID（若存在），否则使用 custom_effects
        if let Some(pc) =
            stack.get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
        {
            // 存在自定义效果
            let mut out = Vec::new();
            if let Some(potion_id) = pc.potion_id {
                // 尽可能将药水 id 映射为生成的 Potion
                macro_rules! try_push_potion {
                    ($p:expr) => {
                        if $p.id as i32 == potion_id {
                            for e in $p.effects {
                                out.push((
                                    e.effect_type,
                                    e.duration,
                                    e.amplifier,
                                    e.ambient,
                                    e.show_particles,
                                    e.show_icon,
                                ));
                            }
                        }
                    };
                }
                try_push_potion!(papokin_data::potion::Potion::AWKWARD);
                try_push_potion!(papokin_data::potion::Potion::FIRE_RESISTANCE);
                try_push_potion!(papokin_data::potion::Potion::HARMING);
                try_push_potion!(papokin_data::potion::Potion::HEALING);
                try_push_potion!(papokin_data::potion::Potion::INFESTED);
                try_push_potion!(papokin_data::potion::Potion::INVISIBILITY);
                try_push_potion!(papokin_data::potion::Potion::LEAPING);
                try_push_potion!(papokin_data::potion::Potion::LONG_FIRE_RESISTANCE);
                try_push_potion!(papokin_data::potion::Potion::LONG_INVISIBILITY);
                try_push_potion!(papokin_data::potion::Potion::LONG_LEAPING);
                try_push_potion!(papokin_data::potion::Potion::LONG_NIGHT_VISION);
                try_push_potion!(papokin_data::potion::Potion::LONG_POISON);
                try_push_potion!(papokin_data::potion::Potion::LONG_REGENERATION);
                try_push_potion!(papokin_data::potion::Potion::LONG_SLOW_FALLING);
                try_push_potion!(papokin_data::potion::Potion::LONG_SLOWNESS);
                try_push_potion!(papokin_data::potion::Potion::LONG_STRENGTH);
                try_push_potion!(papokin_data::potion::Potion::LONG_SWIFTNESS);
                try_push_potion!(papokin_data::potion::Potion::LONG_TURTLE_MASTER);
                try_push_potion!(papokin_data::potion::Potion::LONG_WATER_BREATHING);
                try_push_potion!(papokin_data::potion::Potion::LONG_WEAKNESS);
                try_push_potion!(papokin_data::potion::Potion::LUCK);
                try_push_potion!(papokin_data::potion::Potion::MUNDANE);
                try_push_potion!(papokin_data::potion::Potion::NIGHT_VISION);
                try_push_potion!(papokin_data::potion::Potion::OOZING);
                try_push_potion!(papokin_data::potion::Potion::POISON);
                try_push_potion!(papokin_data::potion::Potion::REGENERATION);
                try_push_potion!(papokin_data::potion::Potion::SLOW_FALLING);
                try_push_potion!(papokin_data::potion::Potion::SLOWNESS);
                try_push_potion!(papokin_data::potion::Potion::STRENGTH);
                try_push_potion!(papokin_data::potion::Potion::STRONG_HARMING);
                try_push_potion!(papokin_data::potion::Potion::STRONG_HEALING);
                try_push_potion!(papokin_data::potion::Potion::STRONG_LEAPING);
                try_push_potion!(papokin_data::potion::Potion::STRONG_POISON);
                try_push_potion!(papokin_data::potion::Potion::STRONG_REGENERATION);
                try_push_potion!(papokin_data::potion::Potion::STRONG_SLOWNESS);
                try_push_potion!(papokin_data::potion::Potion::STRONG_STRENGTH);
                try_push_potion!(papokin_data::potion::Potion::STRONG_SWIFTNESS);
                try_push_potion!(papokin_data::potion::Potion::STRONG_TURTLE_MASTER);
                try_push_potion!(papokin_data::potion::Potion::SWIFTNESS);
                try_push_potion!(papokin_data::potion::Potion::THICK);
                try_push_potion!(papokin_data::potion::Potion::TURTLE_MASTER);
                try_push_potion!(papokin_data::potion::Potion::WATER);
                try_push_potion!(papokin_data::potion::Potion::WATER_BREATHING);
                try_push_potion!(papokin_data::potion::Potion::WEAKNESS);
                try_push_potion!(papokin_data::potion::Potion::WEAVING);
                try_push_potion!(papokin_data::potion::Potion::WIND_CHARGED);
            }

            // 追加了自定义效果
            for ce in &pc.custom_effects {
                if let Some(se) = StatusEffect::from_minecraft_name(&ce.effect_id) {
                    out.push((
                        se,
                        ce.duration,
                        ce.amplifier as u8,
                        ce.ambient,
                        ce.show_particles,
                        ce.show_icon,
                    ));
                }
            }

            return out;
        }

        Vec::new()
    }

    /// 对目标生物实体施加即时或持续效果。
    pub fn apply_effects_to(
        target: &LivingEntity,
        effects: Vec<(&'static StatusEffect, i32, u8, bool, bool, bool)>,
        scale: f32,
        source: PotionApplicationSource,
    ) {
        for (effect_type, duration, amplifier, ambient, show_particles, show_icon) in effects {
            // 即时效果应立即生效
            let is_instant = effect_type.id
                == papokin_data::effect::StatusEffect::INSTANT_HEALTH.id
                || effect_type.id == papokin_data::effect::StatusEffect::INSTANT_DAMAGE.id;

            if is_instant {
                // 即时强度缩放
                let instant_scale = source.instant_scale(scale);

                // 直接应用即时效果逻辑，因为它们不随刻更新
                if effect_type.id == papokin_data::effect::StatusEffect::INSTANT_HEALTH.id {
                    let amount = 4.0 * (1 << amplifier) as f32 * instant_scale;
                    target.heal(amount);
                } else if effect_type.id == papokin_data::effect::StatusEffect::INSTANT_DAMAGE.id {
                    let amount = 6.0 * (1 << amplifier) as f32 * instant_scale;

                    let _ = target.damage(
                        target.get_entity(),
                        amount,
                        papokin_data::damage::DamageType::MAGIC,
                    );
                }

                // 与原版一样，即时效果只应用一次，从不加入活跃效果
                // 效果，它们会在那里残留。
            } else {
                // 时长缩放
                let duration_scale = source.duration_scale(scale);

                let dur = ((duration as f32) * duration_scale).max(1.0) as i32;
                let eff = papokin_data::potion::Effect {
                    effect_type,
                    duration: dur,
                    amplifier,
                    ambient,
                    show_particles,
                    show_icon,
                    blend: false,
                };
                target.add_effect(eff);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PotionApplicationSource;
    use papokin_data::data_component_impl::PotionDurationScaleImpl;
    use papokin_data::item::Item;
    use papokin_data::item_stack::ItemStack;

    #[test]
    fn tipped_arrow_scale_shortens_duration_without_reducing_instant_potency() {
        let tipped_arrow = ItemStack::new(1, &Item::TIPPED_ARROW);
        let scale = tipped_arrow
            .get_data_component::<PotionDurationScaleImpl>()
            .expect("药箭应定义药水时长缩放系数")
            .scale;

        assert_eq!(PotionApplicationSource::Arrow.duration_scale(scale), 0.125);
        assert_eq!(
            (160.0 * PotionApplicationSource::Arrow.duration_scale(scale)) as i32,
            20
        );
        assert_eq!(PotionApplicationSource::Arrow.instant_scale(scale), 1.0);
    }
}
