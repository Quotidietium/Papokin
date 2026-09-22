use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::enchantment::EnchantmentHelper;
use crate::entity::projectile::arrow::{ArrowEntity, ArrowPickup};
use crate::entity::{Entity, EntityBase};
use crate::world::World;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;

/// 可发射弹射物的武器（弓、弩）的抽象，对应原版 `ProjectileWeaponItem`。
pub struct ProjectileWeaponItem;

impl ProjectileWeaponItem {
    pub const ARROW_SPEED_MULTIPLIER: f32 = 3.0;

    #[must_use]
    pub const fn is_arrow(item: &Item) -> bool {
        item.id == Item::ARROW.id
            || item.id == Item::TIPPED_ARROW.id
            || item.id == Item::SPECTRAL_ARROW.id
    }

    #[must_use]
    pub const fn is_arrow_or_firework(item: &Item) -> bool {
        Self::is_arrow(item) || item.id == Item::FIREWORK_ROCKET.id
    }

    /// 创建对应原版 `ProjectileWeaponItem::createProjectile` 的弹射物。
    pub fn create_projectile(
        world: Arc<World>,
        shooter: &Entity,
        weapon: &ItemStack,
        projectile: &ItemStack,
        is_crit: bool,
        is_creative: bool,
    ) -> ArrowEntity {
        let arrow_entity = Entity::new(
            world,
            shooter.pos.load(),
            ArrowEntity::entity_type_for_item(projectile.item),
        );
        let pickup = if is_creative {
            ArrowPickup::CreativeOnly
        } else {
            ArrowPickup::Allowed
        };
        let arrow =
            ArrowEntity::new_shot_with_weapon(arrow_entity, shooter, projectile, weapon, pickup);

        // 应用数据驱动的武器附魔修改
        arrow.set_base_damage(EnchantmentHelper::modify_damage(
            weapon,
            arrow.get_base_damage(),
        ));
        arrow.punch_level.store(
            EnchantmentHelper::modify_knockback(weapon, 0.0) as u8,
            Ordering::Relaxed,
        );
        arrow.set_pierce_level(EnchantmentHelper::process_projectile_piercing(weapon, 0));

        // 数据驱动的 PROJECTILE_SPAWNED 效果（例如 Flame 附魔 -> Ignite 效果）
        arrow.apply_on_projectile_spawned(projectile);

        if is_crit {
            arrow.set_critical(true);
        }

        arrow
    }

    /// 为弹射物武器装填弹药，与原版的 `ProjectileWeaponItem::draw` 一致。
    #[must_use]
    pub fn draw(weapon: &ItemStack, projectile: &ItemStack, is_creative: bool) -> Vec<ItemStack> {
        if projectile.item_count == 0 {
            return Vec::new();
        }

        let num_projectiles = EnchantmentHelper::process_projectile_count(weapon, 1);
        let mut drawn = Vec::with_capacity(num_projectiles);
        let projectile_copy = projectile.copy_with_count(1);

        for i in 0..num_projectiles {
            let drawn_stack = Self::use_ammo(
                weapon,
                if i == 0 { projectile } else { &projectile_copy },
                is_creative,
                i > 0,
            );
            if drawn_stack.item_count > 0 {
                drawn.push(drawn_stack);
            }
        }

        drawn
    }

    /// 处理弹药消耗逻辑，与原版的 `ProjectileWeaponItem::useAmmo` 保持一致。
    #[must_use]
    pub fn use_ammo(
        weapon: &ItemStack,
        projectile: &ItemStack,
        is_creative: bool,
        force_infinite: bool,
    ) -> ItemStack {
        let ammo_to_use = if force_infinite || is_creative {
            0
        } else {
            EnchantmentHelper::process_ammo_use(weapon, projectile, 1)
        };

        if ammo_to_use > projectile.item_count as i32 {
            return ItemStack::EMPTY.clone();
        }

        let mut copy = projectile.copy_with_count(1);
        if ammo_to_use == 0 {
            copy.set_data_component(papokin_data::data_component_impl::IntangibleProjectileImpl);
        }

        copy
    }

    /// 发射弹射物，与原版 `ProjectileWeaponItem::shoot` 一致。
    #[allow(clippy::too_many_arguments)]
    pub fn shoot_projectiles(
        world: &Arc<World>,
        shooter: &Entity,
        weapon: &ItemStack,
        projectiles: &[ItemStack],
        power: f32,
        uncertainty: f32,
        is_crit: bool,
        is_creative: bool,
    ) {
        if projectiles.is_empty() {
            return;
        }

        let max_angle = EnchantmentHelper::process_projectile_spread(weapon, 0.0);
        let angle_step = if projectiles.len() == 1 {
            0.0
        } else {
            2.0 * max_angle / (projectiles.len() as f32 - 1.0)
        };
        let angle_offset = ((projectiles.len() - 1) % 2) as f32 * angle_step / 2.0;
        let mut direction = 1.0f32;

        let (yaw, pitch) = (shooter.yaw.load(), shooter.pitch.load());

        for (i, projectile) in projectiles.iter().enumerate() {
            if projectile.item_count == 0 {
                continue;
            }

            let angle = angle_offset + direction * (i.div_ceil(2) as f32) * angle_step;
            direction = -direction;

            let arrow = Self::create_projectile(
                world.clone(),
                shooter,
                weapon,
                projectile,
                is_crit,
                is_creative,
            );
            arrow.set_velocity_from_rotation(pitch, yaw + angle, 0.0, power, uncertainty);
            let arrow_arc: Arc<dyn EntityBase> = Arc::new(arrow);
            world.spawn_entity(arrow_arc);
        }
    }
}
