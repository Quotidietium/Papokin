use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::item::ItemEntity;
use crate::entity::projectile::{ProjectileHit, is_projectile};
use crate::world::loot::{LootContextParameters, generate_loot_with_context};
use crate::{
    entity::{Entity, EntityBase, living::LivingEntity, player::Player},
    server::Server,
};
use papokin_data::entity::EntityType;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::{Block, Enchantment};
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub struct FishingBobberEntity {
    pub entity: Entity,
    pub owner_id: i32,
    pub hooked_entity_id: AtomicI32,
    pub in_ground: AtomicBool,
    pub has_hit: AtomicBool,
    pub wait_countdown: AtomicI32,
    pub bite_countdown: AtomicI32,
    /// 饵钓附魔带来的等待时长折扣（百分比分子，`100 - 15×等级`），
    /// 抛竿时从鱼竿上捕获（原版在浮漂创建时固化附魔效果）。
    pub lure_speed: i32,
}

impl FishingBobberEntity {
    const WATER_INERTIA: f64 = 0.8;
    const AIR_INERTIA: f64 = 0.92;
    const GRAVITY: f64 = 0.03;

    pub fn new(entity: Entity, owner: &Player) -> Self {
        let mut owner_pos = owner.living_entity.entity.pos.load();
        owner_pos.y += owner.living_entity.entity.get_eye_height() - 0.1;
        entity.pos.store(owner_pos);

        let lure_speed = 100
            - 15 * owner
                .inventory()
                .held_item()
                .get_enchantment_level(&Enchantment::LURE)
                // 附魔等级来自物品组件，可能被指令/客户端注入任意 i32，
                // 先钳制再参与运算，避免溢出 panic（debug 构建下可被利用）
                .clamp(0, 255);
        Self {
            entity,
            owner_id: owner.living_entity.entity.entity_id,
            hooked_entity_id: AtomicI32::new(0),
            in_ground: AtomicBool::new(false),
            has_hit: AtomicBool::new(false),
            wait_countdown: AtomicI32::new(Self::next_wait(lure_speed)),
            bite_countdown: AtomicI32::new(0),
            lure_speed,
        }
    }

    /// 下一次咬钩等待时长：基准均匀 100..=600 刻，按饵钓折扣缩短。
    fn next_wait(lure_speed: i32) -> i32 {
        let base = (rand::random::<i32>().rem_euclid(501)) + 100;
        base * lure_speed.max(10) / 100
    }

    /// 原版的开放水域判定：浮漂周围水平 ±2、竖直 0..=3 的
    /// 5×4×5 区域内只允许空气、液体与睡莲，否则无法钓到宝藏。
    fn is_open_water(&self) -> bool {
        let world = self.entity.world.load();
        let center = self.entity.pos.load();
        let base = BlockPos::new(
            center.x.floor() as i32,
            center.y.floor() as i32,
            center.z.floor() as i32,
        );
        for dx in -2..=2 {
            for dz in -2..=2 {
                for dy in 0..=3 {
                    let pos = base.0 + Vector3::new(dx, dy, dz);
                    let (block, state) = world.get_block_and_state(&BlockPos(pos));
                    if block == &Block::LILY_PAD || state.is_liquid() || state.is_air() {
                        continue;
                    }
                    return false;
                }
            }
        }
        true
    }

    pub fn reel_in(&self, player: &Player) -> i32 {
        let world = self.entity.world.load();
        let hooked_id = self.hooked_entity_id.load(Ordering::Relaxed);

        if hooked_id != 0
            && let Some(hooked) = world.get_entity_by_id(hooked_id)
        {
            let player_pos = player.get_entity().pos.load();
            let hooked_pos = hooked.get_entity().pos.load();
            let delta = player_pos - hooked_pos;
            let motion =
                delta
                    .multiply(0.1, 0.1, 0.1)
                    .add_raw(0.0, delta.length().sqrt() * 0.08, 0.0);
            hooked.get_entity().add_velocity(motion);
            // 原版：钓上实体同样损耗鱼竿耐久
            player.damage_held_item(1);
            return 1;
        }

        if self.bite_countdown.load(Ordering::Relaxed) > 0 {
            // 钓到东西了！
            player.increment_stat(
                papokin_data::statistic::StatisticCategory::Custom,
                papokin_data::statistic::CustomStatistic::FishCaught as i32,
                1,
            );

            // 按原版权重在鱼/垃圾/宝藏三张战利品表中选择：
            // 鱼 85−L、垃圾 10−2L、宝藏 5+2L（L=海之眷顾等级），
            // 宝藏仅在开放水域可钓。
            // 等级先钳制：物品组件可携带任意 i32，防后续运算溢出。
            let luck = player
                .inventory()
                .held_item()
                .get_enchantment_level(&Enchantment::LUCK_OF_THE_SEA)
                .clamp(0, 255);
            let open_water = self.is_open_water();
            let fish_weight = (85 - luck).max(1);
            let junk_weight = (10 - 2 * luck).max(0);
            let treasure_weight = if open_water { (5 + 2 * luck).max(0) } else { 0 };
            let roll =
                rand::random::<i32>().rem_euclid(fish_weight + junk_weight + treasure_weight);
            let table_key = if roll < fish_weight {
                "minecraft:gameplay/fishing/fish"
            } else if roll < fish_weight + junk_weight {
                "minecraft:gameplay/fishing/junk"
            } else {
                "minecraft:gameplay/fishing/treasure"
            };

            // 战利品生成事件，取消则本次垂钓无渔获
            if !world.generate_loot(table_key) {
                return 0;
            }

            let drops = papokin_data::loot_table::get_loot_table(table_key)
                .map(|table| {
                    let params = LootContextParameters {
                        position: Some(self.entity.pos.load()),
                        tool: Some(player.inventory().held_item()),
                        ..Default::default()
                    };
                    let seed: i64 = rand::random();
                    generate_loot_with_context(table, seed, &params)
                })
                .unwrap_or_default();

            if drops.is_empty() {
                return 0;
            }

            // 战利品以物品实体形式从浮漂处飞向玩家（原版弹道）
            let player_pos = player.get_entity().pos.load();
            let bobber_pos = self.entity.pos.load();
            let delta = player_pos - bobber_pos;
            let distance = delta.length();
            let first_item_id = drops
                .first()
                .map(|stack| format!("minecraft:{}", stack.item.registry_key));
            for stack in drops {
                let item_entity = Entity::new(world.clone(), bobber_pos, &EntityType::ITEM);
                let item = ItemEntity::new_with_velocity(
                    item_entity,
                    stack,
                    delta
                        .multiply(0.1, 0.1, 0.1)
                        .add_raw(0.0, distance * 0.08, 0.0),
                    ItemEntity::DEFAULT_PICKUP_DELAY,
                );
                world.spawn_entity(std::sync::Arc::new(item));
            }

            // 原版：钓到鱼掉落 1..=6 经验
            ExperienceOrbEntity::spawn(
                &world,
                player_pos,
                (rand::random::<u32>().rem_euclid(6) + 1) as u32,
            );

            if let Some(first) = first_item_id {
                player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::FishedItem {
                        item_id: first,
                    },
                );
            }

            // 原版：成功收线损耗鱼竿耐久
            player.damage_held_item(1);
            return 1;
        }

        0
    }

    #[expect(clippy::too_many_lines)]
    pub fn process_tick(&self, caller: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load();

        // 主人下线或离开过远时回收浮漂，防止实体泄漏长期占用 tick
        let Some(owner) = world.get_player_by_id(self.owner_id) else {
            caller.get_entity().remove();
            return;
        };
        let owner_pos = owner.get_entity().pos.load();
        if owner_pos.squared_distance_to_vec(&entity.pos.load()) > 32.0 * 32.0 {
            caller.get_entity().remove();
            return;
        }

        if self.in_ground.load(Ordering::Relaxed) {
            return;
        }

        let hooked_id = self.hooked_entity_id.load(Ordering::Relaxed);
        if hooked_id != 0 {
            if let Some(hooked) = world.get_entity_by_id(hooked_id) {
                if hooked.get_entity().removed.load(Ordering::Relaxed) {
                    self.hooked_entity_id.store(0, Ordering::Relaxed);
                } else {
                    let mut hooked_pos = hooked.get_entity().pos.load();
                    hooked_pos.y += hooked.get_entity().get_eye_height() * 0.8;
                    entity.set_pos(hooked_pos);
                    return;
                }
            } else {
                self.hooked_entity_id.store(0, Ordering::Relaxed);
            }
        }

        let mut velocity = entity.velocity.load();
        let start_pos = entity.pos.load();

        if entity.touching_water.load(Ordering::Relaxed) {
            velocity.y += 0.02; // 浮力

            let bite = self.bite_countdown.load(Ordering::Relaxed);
            if bite > 0 {
                self.bite_countdown.store(bite - 1, Ordering::Relaxed);
                if bite % 5 == 0 {
                    world.spawn_particle(
                        entity.pos.load(),
                        Vector3::new(0.1f32, 0.1f32, 0.1f32),
                        0.0,
                        5,
                        papokin_data::particle::Particle::Bubble,
                    );
                }
            } else {
                let wait = self.wait_countdown.load(Ordering::Relaxed);
                if wait > 0 {
                    self.wait_countdown.store(wait - 1, Ordering::Relaxed);
                } else {
                    // 开始咬合
                    self.bite_countdown.store(40, Ordering::Relaxed);
                    self.wait_countdown
                        .store(Self::next_wait(self.lure_speed), Ordering::Relaxed);

                    world.play_sound(
                        Sound::EntityFishingBobberSplash,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                }
            }
        } else {
            velocity.y -= Self::GRAVITY;
        }

        let inertia = if entity.touching_water.load(Ordering::Relaxed) {
            Self::WATER_INERTIA
        } else {
            Self::AIR_INERTIA
        };
        velocity = velocity.multiply(inertia, inertia, inertia);
        entity.velocity.store(velocity);

        let new_pos = start_pos.add(&velocity);

        let search_box = BoundingBox::new(
            Vector3::new(
                start_pos.x.min(new_pos.x),
                start_pos.y.min(new_pos.y),
                start_pos.z.min(new_pos.z),
            ),
            Vector3::new(
                start_pos.x.max(new_pos.x),
                start_pos.y.max(new_pos.y),
                start_pos.z.max(new_pos.z),
            ),
        )
        .expand(0.3, 0.3, 0.3);

        // 基础方块碰撞，用于拦停浮漂
        let (block_cols, _) = world.get_block_collisions(search_box, caller);
        if !block_cols.is_empty() {
            self.in_ground.store(true, Ordering::Relaxed);
            entity.velocity.store(Vector3::new(0.0, 0.0, 0.0));
            return;
        }

        entity.set_pos(new_pos);

        let candidates = world.get_entities_at_box(&search_box);
        for cand in candidates {
            if cand.get_entity().entity_id == self.owner_id
                || cand.get_entity().entity_id == entity.entity_id
            {
                continue;
            }

            if is_projectile(cand.get_entity().entity_type) {
                continue;
            }

            let ebb = cand.get_entity().bounding_box.load().expand(0.3, 0.3, 0.3);
            if ebb.intersects(&search_box) {
                self.hooked_entity_id
                    .store(cand.get_entity().entity_id, Ordering::Relaxed);
                entity.set_synced_data(
                    papokin_data::tracked_data::fishing_bobber::HOOKED_ENTITY,
                    cand.get_entity().entity_id + 1,
                );
                return;
            }
        }
    }
}

impl EntityBase for FishingBobberEntity {
    fn get_owner_id(&self) -> Option<i32> {
        Some(self.owner_id)
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
    fn on_hit(&self, _hit: ProjectileHit) {
        self.has_hit.store(true, Ordering::Relaxed);
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.process_tick(caller);
    }
}
