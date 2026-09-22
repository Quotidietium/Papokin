use std::any::Any;
use std::sync::atomic::Ordering;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::arrow::ArrowEntity;
use crate::item::items::projectile_weapon::ProjectileWeaponItem;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_protocol::IdOr;
use papokin_protocol::java::client::play::CSoundEffect;
use papokin_util::GameMode;

pub struct BowItem;

impl ItemMetadata for BowItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::BOW.id])
    }
}

impl ItemBehaviour for BowItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        // 检查玩家是否有箭（或处于创造模式）
        let has_arrows = Self::has_arrows(player);
        let gamemode = player.gamemode.load();

        if !has_arrows && gamemode != GameMode::Creative {
            return;
        }

        // 获取手持物品堆
        let inventory = player.inventory();
        let stack = inventory.held_item();

        // 拉弓就绪钩子：取消可阻止拉弓开始。
        {
            let world = player.world();
            let arrow = player.find_arrow().map_or_else(
                || ItemStack::new(1, &Item::ARROW),
                |slot| {
                    let mut arrow = inventory.get_slot(slot);
                    arrow.item_count = 1;
                    arrow
                },
            );
            if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id) {
                let mut ready_event = crate::plugin::api::events::player::player_ready_arrow::PlayerReadyArrowEvent::new(
                    player_arc,
                    stack.clone(),
                    arrow,
                );
                if let Some(server) = world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut ready_event);
                }
                if ready_event.cancelled {
                    return;
                }
            }
        }

        // 开始拉弓动画
        player
            .living_entity
            .set_active_hand(papokin_util::Hand::Right, stack, Self::USE_DURATION);
    }

    fn on_stopped_using(&self, stack: &ItemStack, player: &Player) {
        Self::release_bow(player, stack);
    }

    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl BowItem {
    /// 弓可被拉满的最大刻数
    pub const USE_DURATION: i32 = 72000;
    const MAX_DRAW_DURATION: f32 = 20.0;
    pub const ARROW_SPEED_MULTIPLIER: f32 = 3.0;

    /// 当玩家放开弓时调用
    pub fn release_bow(player: &Player, weapon: &ItemStack) {
        // 获取已使用的刻数
        let use_ticks = player.living_entity.item_use_time.load(Ordering::Relaxed);
        let use_ticks = Self::USE_DURATION - use_ticks;

        // 检查最小拉弓时间
        if use_ticks < 3 {
            return;
        }

        // 再次检查箭
        let arrow_slot = player.find_arrow();
        let gamemode = player.gamemode.load();

        if arrow_slot.is_none() && gamemode != GameMode::Creative {
            return;
        }

        let projectile = arrow_slot.map_or_else(
            || ItemStack::new(1, &Item::ARROW),
            |slot| {
                let stack = player.inventory.get_slot(slot);
                stack.copy_with_count(1)
            },
        );
        let infinite_projectile = projectile.item.id == Item::ARROW.id;

        // 计算能量并点燃
        let power = Self::get_power_for_time(use_ticks);

        // 检查无限附魔
        let has_infinity = weapon
            .get_data_component::<papokin_data::data_component_impl::EnchantmentsImpl>()
            .is_some_and(|enchantments| {
                enchantments
                    .enchantment
                    .iter()
                    .any(|(e, _)| **e == papokin_data::Enchantment::INFINITY)
            });

        let is_crit = (power - 1.0).abs() < f32::EPSILON;
        Self::shoot(
            player,
            weapon,
            std::slice::from_ref(&projectile),
            power,
            1.0,
            is_crit,
        );

        // 消耗箭（非创造模式且没有无限附魔时）
        if let Some(slot) = arrow_slot
            && gamemode != GameMode::Creative
            && !(has_infinity && infinite_projectile)
        {
            player.consume_arrow(slot);
        }

        // 损伤弓
        player.damage_held_item(1);
    }

    /// 检查玩家物品栏中是否有箭
    fn has_arrows(player: &Player) -> bool {
        player.find_arrow().is_some()
    }

    /// 根据拉弓时长计算弓的力量/蓄力
    #[must_use]
    pub fn get_power_for_time(time_held: i32) -> f32 {
        let mut power = time_held as f32 / Self::MAX_DRAW_DURATION;
        power = (power * power + power * 2.0) / 3.0;
        if power > 1.0 {
            power = 1.0;
        }
        power
    }

    /// 创建对应原版 `ProjectileWeaponItem::createProjectile` 的弹射物。
    pub fn create_projectile(
        player: &Player,
        weapon: &ItemStack,
        projectile: &ItemStack,
        is_crit: bool,
    ) -> ArrowEntity {
        let world = player.world();
        let is_creative = player.gamemode.load() == GameMode::Creative;
        ProjectileWeaponItem::create_projectile(
            world,
            player.get_entity(),
            weapon,
            projectile,
            is_crit,
            is_creative,
        )
    }

    /// 发射弹射物，与原版 `ProjectileWeaponItem::shoot` 一致。
    pub fn shoot(
        player: &Player,
        weapon: &ItemStack,
        projectiles: &[ItemStack],
        power: f32,
        uncertainty: f32,
        is_crit: bool,
    ) {
        if power < 0.1 || projectiles.is_empty() {
            return;
        }

        let world = player.world();
        let is_creative = player.gamemode.load() == GameMode::Creative;
        let speed = power * ProjectileWeaponItem::ARROW_SPEED_MULTIPLIER;

        ProjectileWeaponItem::shoot_projectiles(
            &world,
            player.get_entity(),
            weapon,
            projectiles,
            speed,
            uncertainty,
            is_crit,
            is_creative,
        );

        let sound_pitch = 1.0 / (rand::random::<f32>() * 0.4 + 1.2) + power * 0.5;
        let sound_packet = CSoundEffect::new(
            IdOr::Id(Sound::EntityArrowShoot as u16),
            SoundCategory::Neutral,
            &player.position(),
            1.0,
            sound_pitch,
            0,
        );
        let chunk_pos = player.get_entity().chunk_pos.load();
        world.broadcast_to_chunk(chunk_pos, &sound_packet);
    }

    /// 从弓中射出一支箭，并显式指定暴击标志
    pub fn fire_arrow_with_crit(
        player: &Player,
        power: f32,
        projectile: &ItemStack,
        is_crit: bool,
    ) {
        let held = player.inventory().held_item();
        Self::shoot(
            player,
            &held,
            std::slice::from_ref(projectile),
            power,
            1.0,
            is_crit,
        );
    }

    /// 从弓中射出一支箭
    pub fn fire_arrow(player: &Player, power: f32, projectile: &ItemStack) {
        Self::fire_arrow_with_crit(player, power, projectile, power >= 1.0);
    }
}
