use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use papokin_data::damage::DamageType;
use papokin_data::effect::StatusEffect;
use papokin_data::entity::EntityType;
use papokin_data::particle::Particle;
use papokin_data::potion::Effect;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_protocol::java::client::play::{CEntityPositionSync, CEntityVelocity};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::mob::shulker::Axis;
use crate::entity::{Entity, EntityBase};
use crate::server::Server;

// Direction 序数常量
const DIR_DOWN: u8 = 0;
const DIR_UP: u8 = 1;
const DIR_NORTH: u8 = 2;
const DIR_SOUTH: u8 = 3;
const DIR_WEST: u8 = 4;
const DIR_EAST: u8 = 5;
const DIR_NONE: u8 = 255;

const SPEED: f64 = 0.15;

pub struct ShulkerBulletEntity {
    pub entity: Entity,
    pub owner_id: i32,
    /// 最终目标的实体 ID；-1 = 无目标，0 及以上 = 有效目标
    target_id: AtomicI32,
    /// 当前移动方向（方向序号，或 `DIR_NONE`）
    current_dir: AtomicU8,
    /// 当前轴段还剩多少刻
    flight_steps: AtomicI32,
    /// 转向速度分量（每个刻更新）
    target_delta_x: AtomicCell<f64>,
    target_delta_y: AtomicCell<f64>,
    target_delta_z: AtomicCell<f64>,
    has_hit: AtomicBool,
    /// 刻计数器；若子弹始终未到达目标，约 150 刻后会被丢弃
    age: AtomicI32,
}

use std::sync::atomic::AtomicU8;

impl ShulkerBulletEntity {
    pub fn new(
        owner: &Entity,
        target_id: i32,
        target_pos: Vector3<f64>,
        invalid_axis: Axis,
    ) -> Self {
        let world = owner.world.load();

        // 位于潜影贝边界框中心的位置
        let bb = owner.bounding_box.load();
        let origin = Vector3::new(
            f64::midpoint(bb.min.x, bb.max.x),
            f64::midpoint(bb.min.y, bb.max.y),
            f64::midpoint(bb.min.z, bb.max.z),
        );

        let entity = Entity::from_uuid(
            Uuid::new_v4(),
            (*world).clone(),
            origin,
            &EntityType::SHULKER_BULLET,
        );

        let bullet = Self {
            entity,
            owner_id: owner.entity_id,
            target_id: AtomicI32::new(target_id),
            current_dir: AtomicU8::new(DIR_UP),
            flight_steps: AtomicI32::new(0),
            target_delta_x: AtomicCell::new(0.0),
            target_delta_y: AtomicCell::new(0.0),
            target_delta_z: AtomicCell::new(0.0),
            has_hit: AtomicBool::new(false),
            age: AtomicI32::new(0),
        };

        // 预置 target_delta 和 flight_steps，让移动从第 1 刻开始。
        let avoid_axis = invalid_axis as u8;
        bullet.select_next_dir(avoid_axis, Some(target_pos));

        bullet
    }

    #[allow(clippy::similar_names)]
    fn select_next_dir(&self, avoid_axis: u8, target_pos_opt: Option<Vector3<f64>>) {
        const Y_OFFSET: f64 = 0.5;

        let pos = self.entity.pos.load();
        let cur_bp = self.entity.block_pos.load();
        let world = self.entity.world.load();

        // 计算目标方块位置
        let tbp = target_pos_opt.map_or_else(
            || BlockPos::new(cur_bp.0.x, cur_bp.0.y - 1, cur_bp.0.z), // 默认：下方
            |tp| {
                BlockPos::new(
                    // 已提供目标
                    tp.x.floor() as i32,
                    (tp.y + Y_OFFSET).floor() as i32,
                    tp.z.floor() as i32,
                )
            },
        );

        // ……以及"转向"目标点。
        let target_x = f64::from(tbp.0.x) + 0.5;
        let target_y = f64::from(tbp.0.y) + Y_OFFSET;
        let target_z = f64::from(tbp.0.z) + 0.5;

        // 如果目标方块中心距弹射物超过 2 格 -> 选择轴向步进。
        let tbp_cx = f64::from(tbp.0.x) + 0.5;
        let tbp_cy = f64::from(tbp.0.y) + 0.5;
        let tbp_cz = f64::from(tbp.0.z) + 0.5;
        let center_dist_sq =
            (tbp_cx - pos.x).powi(2) + (tbp_cy - pos.y).powi(2) + (tbp_cz - pos.z).powi(2);
        let target_is_far = center_dist_sq > 4.0;

        let (chosen_tx, chosen_ty, chosen_tz, selected_dir) = if target_is_far {
            // 选择一个朝向目标的轴对齐方向
            let mut options: Vec<u8> = Vec::new();

            if avoid_axis != 0 {
                if cur_bp.0.x < tbp.0.x && is_empty_block(&world, &cur_bp, DIR_EAST) {
                    options.push(DIR_EAST);
                } else if cur_bp.0.x > tbp.0.x && is_empty_block(&world, &cur_bp, DIR_WEST) {
                    options.push(DIR_WEST);
                }
            }
            if avoid_axis != 1 {
                if cur_bp.0.y < tbp.0.y && is_empty_block(&world, &cur_bp, DIR_UP) {
                    options.push(DIR_UP);
                } else if cur_bp.0.y > tbp.0.y && is_empty_block(&world, &cur_bp, DIR_DOWN) {
                    options.push(DIR_DOWN);
                }
            }
            if avoid_axis != 2 {
                if cur_bp.0.z < tbp.0.z && is_empty_block(&world, &cur_bp, DIR_SOUTH) {
                    options.push(DIR_SOUTH);
                } else if cur_bp.0.z > tbp.0.z && is_empty_block(&world, &cur_bp, DIR_NORTH) {
                    options.push(DIR_NORTH);
                }
            }

            // 如果选项为空，则随机选择一个方向（最多尝试 5 次以找到
            // 无阻挡的方向）；在此回退逻辑中忽略 avoidAxis。
            let sel = if options.is_empty() {
                let mut r = Self::random_dir();
                for _ in 0..4 {
                    if is_empty_block(&world, &cur_bp, r) {
                        break;
                    }
                    r = Self::random_dir();
                }
                r
            } else {
                options[rand::rng().random_range(0..options.len())]
            };

            (
                pos.x + f64::from(dir_step_x_s(sel)),
                pos.y + f64::from(dir_step_y_s(sel)),
                pos.z + f64::from(dir_step_z_s(sel)),
                sel,
            )
        } else {
            // 如果接近，则将 selection 设为 null 并直接瞄准实际目标位置。
            (target_x, target_y, target_z, DIR_NONE)
        };

        // 计算归一化的转向增量并按 SPEED 缩放。
        let xa = chosen_tx - pos.x;
        let ya = chosen_ty - pos.y;
        let za = chosen_tz - pos.z;
        let dist = (xa * xa + ya * ya + za * za).sqrt();

        if dist == 0.0 {
            self.target_delta_x.store(0.0);
            self.target_delta_y.store(0.0);
            self.target_delta_z.store(0.0);
        } else {
            self.target_delta_x.store(xa / dist * SPEED);
            self.target_delta_y.store(ya / dist * SPEED);
            self.target_delta_z.store(za / dist * SPEED);
        }

        self.current_dir.store(selected_dir, Ordering::Relaxed);
        let steps = 10 + rand::rng().random_range(0..5) * 10;
        self.flight_steps.store(steps, Ordering::Relaxed);
    }

    fn random_dir() -> u8 {
        rand::rng().random_range(0u8..6)
    }
}

// 独立步骤辅助函数（用于无法捕获 self.* 的异步闭包中）
const fn dir_step_x_s(dir: u8) -> i32 {
    match dir {
        DIR_EAST => 1,
        DIR_WEST => -1,
        _ => 0,
    }
}
const fn dir_step_y_s(dir: u8) -> i32 {
    match dir {
        DIR_UP => 1,
        DIR_DOWN => -1,
        _ => 0,
    }
}
const fn dir_step_z_s(dir: u8) -> i32 {
    match dir {
        DIR_SOUTH => 1,
        DIR_NORTH => -1,
        _ => 0,
    }
}

///返回方向的轴序数（0=X，1=Y，2=Z）。
/// 对 `DIR_NONE` 或任何未知值返回 255。
const fn dir_axis(dir: u8) -> u8 {
    match dir {
        DIR_EAST | DIR_WEST => 0,
        DIR_UP | DIR_DOWN => 1,
        DIR_NORTH | DIR_SOUTH => 2,
        _ => 255,
    }
}

/// 如果从 `from` 沿 `dir` 方向前进一步后的方块是空气则为 `true`（子弹
/// 可以穿过它）。未加载的区块视为空。
fn is_empty_block(world: &crate::world::World, from: &BlockPos, dir: u8) -> bool {
    let nb = BlockPos::new(
        from.0.x + dir_step_x_s(dir),
        from.0.y + dir_step_y_s(dir),
        from.0.z + dir_step_z_s(dir),
    );
    world
        .get_block_state_if_loaded(&nb)
        .is_none_or(papokin_data::BlockState::is_air)
}

impl ShulkerBulletEntity {
    /// 从预先存在的实体创建 `ShulkerBulletEntity`（例如加载自
    /// 磁盘，或通过 /summon 命令在无上下文的情况下生成）。
    pub const fn orphan(entity: Entity) -> Self {
        Self {
            entity,
            owner_id: 0,
            target_id: AtomicI32::new(-1),
            current_dir: AtomicU8::new(DIR_NONE),
            flight_steps: AtomicI32::new(0),
            target_delta_x: AtomicCell::new(0.0),
            target_delta_y: AtomicCell::new(0.0),
            target_delta_z: AtomicCell::new(0.0),
            has_hit: AtomicBool::new(false),
            age: AtomicI32::new(0),
        }
    }
}

impl EntityBase for ShulkerBulletEntity {
    fn get_owner_id(&self) -> Option<i32> {
        Some(self.owner_id)
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    /// 任何命中都会摧毁弹丸（近战、箭等）。
    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        // 防止重复命中
        if self.has_hit.swap(true, Ordering::SeqCst) {
            return false;
        }
        let entity = &self.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound_fine(
            Sound::EntityShulkerBulletHit,
            SoundCategory::Hostile,
            &pos,
            1.0,
            1.0,
        );
        world.spawn_particle(
            pos,
            Vector3::new(0.2, 0.2, 0.2),
            0.0,
            2,
            Particle::Explosion,
        );
        entity.remove();
        true
    }
    #[allow(clippy::too_many_lines)]
    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        if self.has_hit.load(Ordering::Relaxed) {
            return;
        }

        // 若子弹从未命中任何目标，150 刻后丢弃
        let age = self.age.fetch_add(1, Ordering::Relaxed) + 1;
        if age > 150 {
            if !self.has_hit.swap(true, Ordering::SeqCst) {
                let entity = &self.entity;
                let world = entity.world.load();
                let pos = entity.pos.load();
                world.play_sound_fine(
                    Sound::EntityShulkerBulletHit,
                    SoundCategory::Hostile,
                    &pos,
                    1.0,
                    1.0,
                );
                world.spawn_particle(
                    pos,
                    Vector3::new(0.2, 0.2, 0.2),
                    0.0,
                    2,
                    Particle::Explosion,
                );
                entity.remove();
            }
            return;
        }

        let entity = &self.entity;
        let world = entity.world.load();

        let target_id = self.target_id.load(Ordering::Relaxed);
        let target_opt = if target_id >= 0 {
            world.get_entity_by_id(target_id)
        } else {
            None
        };

        // 仅当目标为空、已死亡或为旁观者时才应用重力。
        let target_alive = target_opt
            .as_ref()
            .is_some_and(|t| t.get_entity().is_alive());

        if !target_alive && target_id >= 0 {
            // 目标 ID 已设置但找不到实体（已死亡/已离开世界）。
            // 只有在确定目标已消失时才永久清除目标。
            if let Some(t) = &target_opt {
                if !t.get_entity().is_alive() {
                    self.target_id.store(-1, Ordering::Relaxed);
                }
            } else {
                // 未找到 -> 清除目标
                self.target_id.store(-1, Ordering::Relaxed);
            }
        }

        if target_alive {
            // 加速目标增量 ×1.025，并做钳制
            let mut tdx = (self.target_delta_x.load() * 1.025).clamp(-1.0, 1.0);
            let mut tdy = (self.target_delta_y.load() * 1.025).clamp(-1.0, 1.0);
            let mut tdz = (self.target_delta_z.load() * 1.025).clamp(-1.0, 1.0);
            self.target_delta_x.store(tdx);
            self.target_delta_y.store(tdy);
            self.target_delta_z.store(tdz);

            // 钳制已初始化的哨兵值
            if tdx.abs() < 1e-10 && tdy.abs() < 1e-10 && tdz.abs() < 1e-10 {
                tdx = 0.0;
                tdy = 0.0;
                tdz = 0.0;
            }

            // 将实际速度向转向增量插值
            let mut vel = entity.velocity.load();
            vel.x += (tdx - vel.x) * 0.2;
            vel.y += (tdy - vel.y) * 0.2;
            vel.z += (tdz - vel.z) * 0.2;
            entity.velocity.store(vel);
        } else {
            // 无活动目标 – 施加重力并漂移
            let mut vel = entity.velocity.load();
            vel.y -= 0.04;
            entity.velocity.store(vel);
        }

        let vel = entity.velocity.load();
        let old_pos = entity.pos.load();
        let new_pos = old_pos.add(&vel);
        entity.set_pos(new_pos);

        // 广播位置与速度
        let chunk_pos = entity.chunk_pos.load();
        world.broadcast_to_chunk(
            chunk_pos,
            &CEntityPositionSync::new(
                entity.entity_id.into(),
                new_pos,
                vel,
                entity.yaw.load(),
                entity.pitch.load(),
                false,
            ),
        );
        world.broadcast_to_chunk(
            chunk_pos,
            &CEntityVelocity::new(entity.entity_id.into(), vel),
        );

        // 检查方块碰撞
        let new_bp = entity.block_pos.load();
        let state = world.get_block_state(&new_bp);
        if !state.is_air() && state.is_solid() {
            if !self.has_hit.swap(true, Ordering::SeqCst) {
                let pos = entity.pos.load();
                world.play_sound_fine(
                    Sound::EntityShulkerBulletHit,
                    SoundCategory::Hostile,
                    &pos,
                    1.0,
                    1.0,
                );
                world.spawn_particle(
                    pos,
                    Vector3::new(0.2, 0.2, 0.2),
                    0.0,
                    2,
                    Particle::Explosion,
                );
                entity.remove();
            }
            return;
        }

        // 检查实体碰撞
        let bullet_bb = entity.bounding_box.load().expand(0.1, 0.1, 0.1);
        let nearby_entities = world.get_entities_at_box(&bullet_bb);
        let nearby_players = world.get_players_at_box(&bullet_bb);
        let nearby: Vec<Arc<dyn crate::entity::EntityBase>> = nearby_entities
            .into_iter()
            .chain(
                nearby_players
                    .into_iter()
                    .map(|p| p as Arc<dyn crate::entity::EntityBase>),
            )
            .collect();
        for hit_entity in nearby {
            let he = hit_entity.get_entity();
            // 跳过自身
            if he.entity_id == entity.entity_id {
                continue;
            }
            // 永不命中主人的潜影贝
            if he.entity_id == self.owner_id {
                continue;
            }
            // 必须是存活状态
            if !he.is_alive() {
                continue;
            }
            // 必须是生物实体
            let Some(living) = hit_entity.get_living_entity() else {
                continue;
            };
            if !living.entity.is_alive() {
                continue;
            }

            if self.has_hit.swap(true, Ordering::SeqCst) {
                break;
            }

            // 造成 4 点（MOB_PROJECTILE）伤害
            let owner_arc = world.get_entity_by_id(self.owner_id);
            let damaged = hit_entity.damage_with_context(
                hit_entity.as_ref(),
                4.0,
                DamageType::MOB_PROJECTILE,
                None,
                owner_arc.as_deref(),
                None,
            );

            if damaged && let Some(living) = hit_entity.get_living_entity() {
                // 施加 200 刻的飘浮效果
                living.add_effect(Effect {
                    effect_type: &StatusEffect::LEVITATION,
                    duration: 200,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }

            let pos = entity.pos.load();
            world.spawn_particle(
                pos,
                Vector3::new(0.2, 0.2, 0.2),
                0.0,
                2,
                Particle::Explosion,
            );
            entity.remove();
            break;
        }

        if !target_alive || self.has_hit.load(Ordering::Relaxed) {
            return;
        }

        let target_pos = target_opt.as_ref().map(|t| t.get_entity().pos.load());

        let raw_dir = self.current_dir.load(Ordering::Relaxed);
        let avoid_axis = dir_axis(raw_dir);

        // 递减飞行步数计数器；归零时重新选择方向
        let steps = self.flight_steps.fetch_sub(1, Ordering::Relaxed) - 1;
        if steps <= 0 {
            self.select_next_dir(avoid_axis, target_pos);
        }

        // 检查紧邻前方的方块。
        // 如果它是实体则必须重新选择；如果已将坐标轴与目标对齐，也要重新选择。
        let dir = self.current_dir.load(Ordering::Relaxed);
        if dir != DIR_NONE {
            let cur_bp = entity.block_pos.load();
            if !is_empty_block(&world, &cur_bp, dir) {
                // 实心障碍物 -> 绕行
                self.select_next_dir(dir_axis(dir), target_pos);
            } else if let Some(tp) = target_pos {
                let axis = dir_axis(dir);
                let tbp = BlockPos::new(
                    tp.x.floor() as i32,
                    tp.y.floor() as i32,
                    tp.z.floor() as i32,
                );
                let cur_bp2 = entity.block_pos.load();
                let reached = match axis {
                    0 => cur_bp2.0.x == tbp.0.x,
                    1 => cur_bp2.0.y == tbp.0.y,
                    _ => cur_bp2.0.z == tbp.0.z,
                };
                if reached {
                    // 已在此轴对齐 -> 切换到次优轴
                    self.select_next_dir(axis, Some(tp));
                }
            }
        }
    }
}
