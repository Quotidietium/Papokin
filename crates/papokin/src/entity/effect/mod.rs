pub mod hunger;
pub mod infested;
pub mod oozing;
pub mod poison;
pub mod raid_omen;
pub mod regeneration;
pub mod saturation;
pub mod weaving;
pub mod wind_charged;
pub mod wither;

use papokin_data::damage::DamageType;
use papokin_data::effect::StatusEffect;
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use tracing::warn;

use crate::entity::living::LivingEntity;
use crate::entity::{NBTStorage, NBTStorageInit};

pub trait MobEffect: Send + Sync {
    /// 若当前刻与持续时间下应调用 `apply_effect_tick`，则返回 true。
    fn should_apply_effect_tick(&self, _duration: i32, _amplifier: u8) -> bool {
        false
    }

    /// 对生物实体应用周期性/基于刻的效果逻辑。
    fn apply_effect_tick(&self, _living: &LivingEntity, _amplifier: u8) {}

    /// 当携带此效果的实体受伤时调用。
    fn on_mob_hurt(
        &self,
        _living: &LivingEntity,
        _amplifier: u8,
        _damage_type: &DamageType,
        _damage_amount: f32,
    ) {
    }

    /// 当携带此效果的实体死亡时调用。
    fn on_mob_death(&self, _living: &LivingEntity, _amplifier: u8, _damage_type: &DamageType) {}
}

pub static REGENERATION: regeneration::RegenerationMobEffect = regeneration::RegenerationMobEffect;
pub static POISON: poison::PoisonMobEffect = poison::PoisonMobEffect;
pub static WITHER: wither::WitherMobEffect = wither::WitherMobEffect;
pub static HUNGER: hunger::HungerMobEffect = hunger::HungerMobEffect;
pub static SATURATION: saturation::SaturationMobEffect = saturation::SaturationMobEffect;
pub static RAID_OMEN: raid_omen::RaidOmenMobEffect = raid_omen::RaidOmenMobEffect;
pub static INFESTED: infested::InfestedMobEffect = infested::InfestedMobEffect;
pub static OOZING: oozing::OozingMobEffect = oozing::OozingMobEffect;
pub static WEAVING: weaving::WeavingMobEffect = weaving::WeavingMobEffect;
pub static WIND_CHARGED: wind_charged::WindChargedMobEffect = wind_charged::WindChargedMobEffect;

#[must_use]
pub fn get_mob_effect(effect: &'static StatusEffect) -> Option<&'static dyn MobEffect> {
    if effect == &StatusEffect::REGENERATION {
        Some(&REGENERATION)
    } else if effect == &StatusEffect::POISON {
        Some(&POISON)
    } else if effect == &StatusEffect::WITHER {
        Some(&WITHER)
    } else if effect == &StatusEffect::HUNGER {
        Some(&HUNGER)
    } else if effect == &StatusEffect::SATURATION {
        Some(&SATURATION)
    } else if effect == &StatusEffect::RAID_OMEN {
        Some(&RAID_OMEN)
    } else if effect == &StatusEffect::INFESTED {
        Some(&INFESTED)
    } else if effect == &StatusEffect::OOZING {
        Some(&OOZING)
    } else if effect == &StatusEffect::WEAVING {
        Some(&WEAVING)
    } else if effect == &StatusEffect::WIND_CHARGED {
        Some(&WIND_CHARGED)
    } else {
        None
    }
}

impl NBTStorage for papokin_data::potion::Effect {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put("id", self.effect_type.minecraft_name);
        if self.amplifier > 0 {
            nbt.put("amplifier", NbtTag::Int(i32::from(self.amplifier)));
        }
        nbt.put("duration", NbtTag::Int(self.duration));
        if self.ambient {
            nbt.put("ambient", NbtTag::Byte(1));
        }
        if !self.show_particles {
            nbt.put("show_particles", NbtTag::Byte(0));
        }
        let show_icon: i8 = i8::from(self.show_icon);
        nbt.put("show_icon", NbtTag::Byte(show_icon));
    }
}

impl NBTStorageInit for papokin_data::potion::Effect {
    fn create_from_nbt(nbt: &mut NbtCompound) -> Option<Self> {
        let Some(effect_id) = nbt.get_string("id") else {
            warn!("无法读取效果：效果 id 不存在");
            return None;
        };
        let Some(effect_type) = StatusEffect::from_minecraft_name(effect_id) else {
            warn!("无法读取效果：未知的效果类型 {effect_id}");
            return None;
        };
        let Some(show_icon) = nbt.get_byte("show_icon") else {
            warn!("无法读取效果：show_icon 不存在");
            return None;
        };
        let amplifier = nbt.get_int("amplifier").unwrap_or(0) as u8;
        let duration = nbt.get_int("duration").unwrap_or(0);
        let ambient = nbt.get_byte("ambient").unwrap_or(0) == 1;
        let show_particles = nbt.get_byte("show_particles").unwrap_or(1) == 1;
        let show_icon = show_icon == 1;
        Some(Self {
            effect_type,
            duration,
            amplifier,
            ambient,
            show_particles,
            show_icon,
            blend: false,
        })
    }
}
