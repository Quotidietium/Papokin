use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::{
    entity::{Entity, EntityBase, living::LivingEntity, player::Player},
    server::Server,
};
use papokin_data::damage::DamageType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_protocol::IdOr;
use papokin_protocol::java::client::play::{CEntityVelocity, CSoundEffect};
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

use super::arrow::ArrowPickup;
use super::{ProjectileHit, calculate_ray_intersection};

pub struct TridentEntity {
    pub entity: Entity,
    pub owner_id: Option<i32>,
    pub item_stack: Arc<Mutex<ItemStack>>,
    pub pickup: ArrowPickup,
    pub in_ground: AtomicBool,
    pub in_ground_time: AtomicU32,
    pub life: AtomicU32,
    pub shake_time: AtomicU8,
    pub has_hit: AtomicBool,
    pub last_block_pos: Arc<std::sync::RwLock<Option<BlockPos>>>,
}

impl TridentEntity {
    const BASE_DAMAGE: f64 = 8.0;
    const AIR_INERTIA: f64 = 0.99;
    const WATER_INERTIA: f64 = 0.9;
    const GRAVITY: f64 = 0.05;
    const DESPAWN_TIME: u32 = 1200;

    pub fn new(entity: Entity, owner_id: Option<i32>) -> Self {
        Self {
            entity,
            owner_id,
            item_stack: Arc::new(Mutex::new(ItemStack::new(1, &Item::TRIDENT))),
            pickup: ArrowPickup::Disallowed,
            in_ground: AtomicBool::new(false),
            in_ground_time: AtomicU32::new(0),
            life: AtomicU32::new(0),
            shake_time: AtomicU8::new(0),
            has_hit: AtomicBool::new(false),
            last_block_pos: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn new_shot(
        entity: Entity,
        shooter: &Entity,
        item_stack: ItemStack,
        pickup: ArrowPickup,
    ) -> Self {
        let mut owner_pos = shooter.pos.load();
        owner_pos.y = owner_pos.y + f64::from(shooter.entity_dimension.load().eye_height) - 0.1;
        entity.pos.store(owner_pos);
        entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));

        Self {
            entity,
            owner_id: Some(shooter.entity_id),
            item_stack: Arc::new(Mutex::new(item_stack)),
            pickup,
            in_ground: AtomicBool::new(false),
            in_ground_time: AtomicU32::new(0),
            life: AtomicU32::new(0),
            shake_time: AtomicU8::new(0),
            has_hit: AtomicBool::new(false),
            last_block_pos: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// 应用弹射物生成类附魔效果，与原版 `Projectile::applyOnProjectileSpawned` 一致。
    pub fn apply_on_projectile_spawned(&self, pickup_item_stack: &ItemStack) {
        super::apply_on_projectile_spawned(self.get_entity(), pickup_item_stack, None, None);
    }

    pub fn set_velocity_from_rotation(
        &self,
        pitch: f32,
        yaw: f32,
        roll: f32,
        speed: f32,
        divergence: f32,
    ) {
        let yaw_rad = yaw.to_radians();
        let pitch_rad = pitch.to_radians();
        let roll_rad = (pitch + roll).to_radians();

        let x = -yaw_rad.sin() * pitch_rad.cos();
        let y = -roll_rad.sin();
        let z = yaw_rad.cos() * pitch_rad.cos();

        self.set_velocity(
            f64::from(x),
            f64::from(y),
            f64::from(z),
            f64::from(speed),
            f64::from(divergence),
        );
    }

    pub fn set_velocity(&self, x: f64, y: f64, z: f64, power: f64, uncertainty: f64) {
        fn next_triangular(mode: f64, deviation: f64) -> f64 {
            deviation.mul_add(rand::random::<f64>() - rand::random::<f64>(), mode)
        }

        let velocity = Vector3::new(x, y, z)
            .normalize()
            .add_raw(
                next_triangular(0.0, 0.017_227_5 * uncertainty),
                next_triangular(0.0, 0.017_227_5 * uncertainty),
                next_triangular(0.0, 0.017_227_5 * uncertainty),
            )
            .multiply(power, power, power);

        self.entity.velocity.store(velocity);
        let len = velocity.horizontal_length();
        self.entity.set_rotation(
            velocity.x.atan2(velocity.z) as f32 * 57.295_776,
            velocity.y.atan2(len) as f32 * 57.295_776,
        );
    }

    fn should_skip_collision(&self, self_ent: &Entity, other: &Arc<dyn EntityBase>) -> bool {
        let other_ent = other.get_entity();

        // 不要与自身碰撞
        if other_ent.entity_id == self_ent.entity_id {
            return true;
        }

        // 对初始帧跳过拥有者（5 刻）
        if Some(other_ent.entity_id) == self.owner_id && self_ent.age.load(Ordering::Relaxed) < 5 {
            return true;
        }

        // 跳过其他弹射物和物品实体
        if other_ent.entity_type == &papokin_data::entity::EntityType::ARROW
            || other_ent.entity_type == &papokin_data::entity::EntityType::TRIDENT
            || other_ent.entity_type == &papokin_data::entity::EntityType::ITEM
            || other_ent.entity_type == &papokin_data::entity::EntityType::FALLING_BLOCK
        {
            return true;
        }

        false
    }
}

impl EntityBase for TridentEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.owner_id
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = self.get_entity();
        let world = entity.world.load();

        // 处理震动时间
        let shake = self.shake_time.load(Ordering::Relaxed);
        if shake > 0 {
            self.shake_time.store(shake - 1, Ordering::Relaxed);
        }

        if self.in_ground.load(Ordering::Relaxed) {
            let _in_ground_time = self.in_ground_time.fetch_add(1, Ordering::Relaxed);
            let life = self.life.fetch_add(1, Ordering::Relaxed);

            // 足够时间后消失
            if life >= Self::DESPAWN_TIME {
                entity.remove();
            }
            return;
        }

        // 三叉戟正在飞行
        let start_pos = entity.pos.load();
        let mut velocity = entity.velocity.load();

        // 应用重力
        velocity.y -= Self::GRAVITY;

        // 应用惯性（空气阻力或水的阻力）
        let inertia = if entity.touching_water.load(Ordering::Relaxed) {
            Self::WATER_INERTIA
        } else {
            Self::AIR_INERTIA
        };
        velocity = velocity.multiply(inertia, inertia, inertia);

        entity.velocity.store(velocity);

        // 根据速度更新旋转
        let len = velocity.horizontal_length();
        entity.set_rotation(
            velocity.x.atan2(velocity.z) as f32 * 57.295_776,
            velocity.y.atan2(len) as f32 * 57.295_776,
        );

        // 移动三叉戟
        let new_pos = start_pos.add(&velocity);
        entity.set_pos(new_pos);

        // 广播速度更新
        let packet = CEntityVelocity::new(entity.entity_id.into(), velocity);
        let chunk_pos = entity.chunk_pos.load();
        world.broadcast_to_chunk(chunk_pos, &packet);

        // 使用射线检测检查碰撞
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

        let mut closest_t = 1.0f64;
        let mut hit = None;

        // 方块碰撞
        let (block_cols, block_positions) =
            world.get_block_collisions(search_box, self.get_entity());
        for (idx, bb) in block_cols.iter().enumerate() {
            if let Some(t) = calculate_ray_intersection(&start_pos, &velocity, bb)
                && t < closest_t
            {
                closest_t = t;

                // 映射回方块坐标
                let mut curr = 0;
                for (len, pos) in &block_positions {
                    curr += len;
                    if idx < curr {
                        let hit_pos = start_pos.add(&velocity.multiply(t, t, t));
                        hit = Some(ProjectileHit::Block {
                            pos: *pos,
                            face: get_hit_face(hit_pos, *pos),
                            hit_pos,
                            normal: velocity.normalize().multiply(-1.0, -1.0, -1.0),
                        });
                        break;
                    }
                }
            }
        }

        // 实体碰撞
        let candidates = world.get_entities_at_box(&search_box);
        for cand in candidates {
            if self.should_skip_collision(entity, &cand) {
                continue;
            }

            let ebb = cand.get_entity().bounding_box.load().expand(0.3, 0.3, 0.3);
            if let Some(t) = calculate_ray_intersection(&start_pos, &velocity, &ebb)
                && t < closest_t
            {
                closest_t = t;
                let hit_pos = start_pos.add(&velocity.multiply(t, t, t));
                hit = Some(ProjectileHit::Entity {
                    entity: cand.clone(),
                    hit_pos,
                    normal: velocity.normalize().multiply(-1.0, -1.0, -1.0),
                });
            }
        }

        // 处理命中
        if let Some(h) = hit
            && !self.has_hit.swap(true, Ordering::SeqCst)
        {
            caller.on_hit(h);
        }
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

    fn on_hit(&self, hit: ProjectileHit) {
        let entity = self.get_entity();
        let world = entity.world.load();

        match hit {
            ProjectileHit::Block { pos, hit_pos, .. } => {
                self.in_ground.store(true, Ordering::Relaxed);
                self.shake_time.store(7, Ordering::Relaxed);
                *self
                    .last_block_pos
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pos);

                let block = world.get_block(&pos);
                let state = world.get_block_state(&pos);
                if let Some(server) = world.server.upgrade() {
                    world
                        .block_registry
                        .on_projectile_hit(block, &world, self, &pos, state, &hit_pos, &server);
                }

                // 使三叉戟停止
                entity.velocity.store(Vector3::new(0.0, 0.0, 0.0));
                entity.set_pos(hit_pos);

                // 播放音效
                let sound_packet = CSoundEffect::new(
                    IdOr::Id(Sound::ItemTridentHitGround as u16),
                    SoundCategory::Neutral,
                    &hit_pos,
                    1.0,
                    1.0,
                    0,
                );
                let chunk_pos = entity.chunk_pos.load();
                world.broadcast_to_chunk(chunk_pos, &sound_packet);
            }
            ProjectileHit::Entity {
                entity: target,
                hit_pos,
                ..
            } => {
                let mut damage = Self::BASE_DAMAGE;

                // 应用穿刺附魔的额外伤害
                if let Some(enchantments) = self
                    .item_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get_data_component::<papokin_data::data_component_impl::EnchantmentsImpl>()
                {
                    for (enchantment, level) in enchantments.enchantment.iter() {
                        if **enchantment == papokin_data::Enchantment::IMPALING {
                            let in_water =
                                target.get_entity().touching_water.load(Ordering::Relaxed);
                            if in_water {
                                damage += 1.25 * f64::from(*level);
                            }
                        }
                    }
                }

                let damage_val = damage as f32;
                target.damage(&*target, damage_val, DamageType::TRIDENT);

                // 播放命中音效
                let sound_packet = CSoundEffect::new(
                    IdOr::Id(Sound::ItemTridentHit as u16),
                    SoundCategory::Neutral,
                    &hit_pos,
                    1.0,
                    1.0,
                    0,
                );
                let chunk_pos = entity.chunk_pos.load();
                world.broadcast_to_chunk(chunk_pos, &sound_packet);

                // 标准的弹跳/回落行为
                entity.velocity.store(Vector3::new(0.0, -0.1, 0.0));
                self.has_hit.store(false, Ordering::Relaxed); // 让它落到地面
            }
        }
    }

    fn on_player_collision(&self, player: &Arc<Player>) {
        // 仅在地面上时才能拾取
        if !self.in_ground.load(Ordering::Relaxed) {
            return;
        }

        if player.living_entity.health.load() <= 0.0 {
            return;
        }

        match self.pickup {
            ArrowPickup::Disallowed => return,
            ArrowPickup::CreativeOnly if !player.is_creative() => return,
            _ => {}
        }

        let mut stack = self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if player.is_creative() || player.inventory.insert_stack_anywhere(&mut stack) {
            player.increment_stat(
                papokin_data::statistic::StatisticCategory::PickedUp,
                stack.item.id as i32,
                1,
            );
            player.living_entity.pickup(&self.entity, 1);
            self.get_entity().remove();
        }
    }
}

/// 获取被击中方块的面
fn get_hit_face(hit_pos: Vector3<f64>, block_pos: BlockPos) -> papokin_data::BlockDirection {
    let local = hit_pos.sub(&block_pos.0.to_f64());
    let eps = 1.0e-4;

    if local.x <= eps {
        papokin_data::BlockDirection::West
    } else if local.x >= 1.0 - eps {
        papokin_data::BlockDirection::East
    } else if local.y <= eps {
        papokin_data::BlockDirection::Down
    } else if local.y >= 1.0 - eps {
        papokin_data::BlockDirection::Up
    } else if local.z <= eps {
        papokin_data::BlockDirection::North
    } else {
        papokin_data::BlockDirection::South
    }
}
