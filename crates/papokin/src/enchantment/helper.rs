use crate::enchantment::effects::EnchantmentEntityEffectExt;
use crate::entity::Entity;
use crate::entity::projectile::arrow::ArrowEntity;
use papokin_data::data_component_impl::EnchantmentsImpl;
use papokin_data::enchantment::{Enchantment, EnchantmentEntityEffect, EnchantmentTarget};
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;

/// 对应原版 `EnchantmentHelper` 的数据驱动附魔效果助手。
pub struct EnchantmentHelper;

impl EnchantmentHelper {
    /// 遍历物品堆上的附魔，与原版 `runIterationOnItem` 保持一致。
    pub fn run_iteration_on_item<F>(item_stack: &ItemStack, mut visitor: F)
    where
        F: FnMut(&'static Enchantment, i32),
    {
        if let Some(enchantments) = item_stack.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                visitor(enchantment, *level);
            }
        }
    }

    /// 应用弹射物生成类附魔效果（如火矢点燃弹射物 100 刻）。
    pub fn on_projectile_spawned(
        weapon: &ItemStack,
        projectile_entity: &Entity,
        arrow: Option<&ArrowEntity>,
    ) {
        let world = projectile_entity.world.load_full();
        Self::run_iteration_on_item(weapon, |enchantment, level| {
            for conditional_effect in enchantment.effects.projectile_spawned {
                conditional_effect.effect.apply(
                    &world,
                    level,
                    None,
                    Some(projectile_entity),
                    projectile_entity.pos.load(),
                );
                if let EnchantmentEntityEffect::Ignite { .. } = &conditional_effect.effect
                    && let Some(arrow) = arrow
                {
                    arrow.set_flame(true);
                }
            }
        });
    }

    /// 应用攻击后附魔效果（如火焰附加点燃受害者）。
    pub fn on_post_attack(attacker: &Entity, victim: &Entity, weapon: &ItemStack) {
        Self::run_iteration_on_item(weapon, |enchantment, level| {
            for targeted_effect in enchantment.effects.post_attack {
                let target = match targeted_effect.affected {
                    Some(EnchantmentTarget::Attacker | EnchantmentTarget::DamagingEntity) => {
                        attacker
                    }
                    Some(EnchantmentTarget::Victim) | None => victim,
                };
                let world = target.world.load_full();
                targeted_effect
                    .effect
                    .apply(&world, level, None, Some(target), target.pos.load());
            }
        });
    }

    /// 使用数据驱动的弹射物散布效果计算弹射物散布角度。
    #[must_use]
    pub fn process_projectile_spread(weapon: &ItemStack, base_spread: f32) -> f32 {
        let mut spread = base_spread;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_projectile_spread(*level, &mut spread);
            }
        }
        spread
    }

    /// 使用数据驱动的弹射物数量效果计算弹射物数量。
    #[must_use]
    pub fn process_projectile_count(weapon: &ItemStack, base_count: usize) -> usize {
        let mut count = base_count as f32;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_projectile_count(*level, &mut count);
            }
        }
        count as usize
    }

    /// 使用数据驱动的穿透效果计算弹射物穿透等级。
    #[must_use]
    pub fn process_projectile_piercing(weapon: &ItemStack, base_piercing: u8) -> u8 {
        let mut piercing = f32::from(base_piercing);
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_piercing_count(*level, &mut piercing);
            }
        }
        piercing as u8
    }

    /// 使用数据驱动的弹药消耗效果（例如无限）计算弹药消耗量。
    #[must_use]
    pub fn process_ammo_use(weapon: &ItemStack, projectile: &ItemStack, base_ammo: i32) -> i32 {
        let mut ammo = base_ammo as f32;
        if projectile.item.id == Item::ARROW.id
            && let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>()
        {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_ammo_count(*level, &mut ammo);
            }
        }
        ammo as i32
    }

    /// 使用数据驱动的伤害效果修改伤害（例如力量/锋利）。
    #[must_use]
    pub fn modify_damage(weapon: &ItemStack, base_damage: f64) -> f64 {
        let mut damage = base_damage;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_damage(*level, &mut damage);
            }
        }
        damage
    }

    /// 使用数据驱动的效果修改重击/坠落伤害（例如致密）。
    #[must_use]
    pub fn modify_fall_based_damage(weapon: &ItemStack, base_damage: f64) -> f64 {
        let mut damage = base_damage;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_fall_based_damage(*level, &mut damage);
            }
        }
        damage
    }

    /// 使用数据驱动的击退效果修改击退（例如冲击/击退）。
    #[must_use]
    pub fn modify_knockback(weapon: &ItemStack, base_knockback: f32) -> f32 {
        let mut knockback = base_knockback;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_knockback(*level, &mut knockback);
            }
        }
        knockback
    }

    /// 使用数据驱动的效果修改盔甲的有效性（例如破甲）。
    #[must_use]
    pub fn modify_armor_effectiveness(weapon: &ItemStack, base_effectiveness: f32) -> f32 {
        let mut effectiveness = base_effectiveness;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_armor_effectiveness(*level, &mut effectiveness);
            }
        }
        effectiveness
    }

    /// 使用数据驱动的装填时间效果修改弩的装填时间（例如快速装填）。
    #[must_use]
    pub fn modify_crossbow_charge_time(weapon: &ItemStack, base_ticks: i32) -> i32 {
        let mut charge_time = base_ticks as f32;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                let mut change_sec = 0.0f32;
                enchantment.modify_crossbow_charge_time(*level, &mut change_sec);
                charge_time += change_sec * 20.0;
            }
        }
        (charge_time as i32).max(0)
    }

    /// 使用数据驱动的物品损伤效果修改耐久度变化（例如耐久）。
    #[must_use]
    pub fn modify_durability_change(item: &ItemStack, base_change: f32) -> f32 {
        let mut change = base_change;
        if let Some(enchantments) = item.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                for effect in enchantment.effects.item_damage {
                    change = effect.effect.process(*level, change);
                }
            }
        }
        change
    }

    /// 使用数据驱动的方块经验效果修改方块经验（例如时运）。
    #[must_use]
    pub fn modify_block_experience(tool: &ItemStack, base_xp: i32) -> i32 {
        let mut xp = base_xp as f32;
        if let Some(enchantments) = tool.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_block_experience(*level, &mut xp);
            }
        }
        (xp as i32).max(0)
    }

    /// 使用数据驱动的生物经验效果修改生物经验（例如抢夺）。
    #[must_use]
    pub fn modify_mob_experience(weapon: &ItemStack, base_xp: i32) -> i32 {
        let mut xp = base_xp as f32;
        if let Some(enchantments) = weapon.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_mob_experience(*level, &mut xp);
            }
        }
        (xp as i32).max(0)
    }

    /// 使用数据驱动的效果修改耐久度以通过经验修复（例如经验修补）。
    #[must_use]
    pub fn modify_durability_to_repair_from_xp(item: &ItemStack, base_repair: f32) -> f32 {
        let mut repair = base_repair;
        if let Some(enchantments) = item.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_durability_to_repair_from_xp(*level, &mut repair);
            }
        }
        repair
    }

    /// 使用数据驱动的效果修改三叉戟返回加速度（例如忠诚）。
    #[must_use]
    pub fn modify_trident_return_acceleration(trident: &ItemStack, base_accel: f32) -> f32 {
        let mut accel = base_accel;
        if let Some(enchantments) = trident.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_trident_return_to_owner_acceleration(*level, &mut accel);
            }
        }
        accel
    }

    /// 使用数据驱动的效果修改三叉戟旋转攻击强度（例如激流）。
    #[must_use]
    pub fn modify_trident_spin_attack_strength(trident: &ItemStack, base_strength: f32) -> f32 {
        let mut strength = base_strength;
        if let Some(enchantments) = trident.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_trident_spin_attack_strength(*level, &mut strength);
            }
        }
        strength
    }

    /// 使用数据驱动的效果修改钓鱼时间缩减（例如饵钓）。
    #[must_use]
    pub fn modify_fishing_time_reduction(rod: &ItemStack, base_reduction: f32) -> f32 {
        let mut reduction = base_reduction;
        if let Some(enchantments) = rod.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_fishing_time_reduction(*level, &mut reduction);
            }
        }
        reduction
    }

    /// 使用数据驱动的效果修改钓鱼幸运加成（例如海之眷顾）。
    #[must_use]
    pub fn modify_fishing_luck_bonus(rod: &ItemStack, base_luck: f32) -> f32 {
        let mut luck = base_luck;
        if let Some(enchantments) = rod.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                enchantment.modify_fishing_luck_bonus(*level, &mut luck);
            }
        }
        luck
    }

    /// 修改已装备盔甲提供的伤害保护。
    #[must_use]
    pub fn modify_damage_protection<'a>(
        armor_items: impl IntoIterator<Item = &'a ItemStack>,
        base_protection: f32,
    ) -> f32 {
        let mut protection = base_protection;
        for item in armor_items {
            if let Some(enchantments) = item.get_data_component::<EnchantmentsImpl>() {
                for (enchantment, level) in enchantments.enchantment.iter() {
                    for effect in enchantment.effects.damage_protection {
                        protection = effect.effect.process(*level, protection);
                    }
                }
            }
        }
        protection
    }

    /// 应用位置变化类附魔效果（如冰霜行者将水替换为浮冰）。
    pub fn on_location_changed(
        entity: &Entity,
        item: &ItemStack,
        position: papokin_util::math::vector3::Vector3<f64>,
    ) {
        let world = entity.world.load_full();
        Self::run_iteration_on_item(item, |enchantment, level| {
            for conditional_effect in enchantment.effects.location_changed {
                conditional_effect
                    .effect
                    .apply(&world, level, None, Some(entity), position);
            }
        });
    }

    /// 应用命中方块类附魔效果。
    pub fn on_hit_block(
        weapon: &ItemStack,
        projectile_entity: &Entity,
        position: papokin_util::math::vector3::Vector3<f64>,
    ) {
        let world = projectile_entity.world.load_full();
        Self::run_iteration_on_item(weapon, |enchantment, level| {
            for conditional_effect in enchantment.effects.hit_block {
                conditional_effect.effect.apply(
                    &world,
                    level,
                    None,
                    Some(projectile_entity),
                    position,
                );
            }
        });
    }
}
