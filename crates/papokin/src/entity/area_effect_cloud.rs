use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{
    entity::{Entity, EntityBase},
    server::Server,
};
use papokin_data::effect::StatusEffect;
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::vector3::Vector3;

type EffectEntry = (&'static StatusEffect, i32, u8, bool, bool, bool);
use papokin_data::item_stack::ItemStack;

#[derive(Clone)]
struct ParticleMeta {
    particle_id: papokin_protocol::codec::var_int::VarInt,
    data: Box<[u8]>,
}

impl papokin_protocol::java::client::play::MetadataSerializer for ParticleMeta {
    fn write_metadata(
        &self,
        writer: &mut impl std::io::Write,
        _version: &papokin_util::version::JavaMinecraftVersion,
    ) -> Result<(), papokin_protocol::ser::WritingError> {
        use papokin_protocol::ser::NetworkWriteExt;
        writer.write_var_int(&self.particle_id)?;
        writer.write_slice(&self.data)
    }
}

/// 滞留药水落地时生成的效果云实体。
pub struct AreaEffectCloudEntity {
    pub entity: Entity,
    /// 从中读取效果所存储的药水物品堆（可能为空）。
    pub item_stack: Mutex<ItemStack>,
    /// 活跃的药水效果，以元组表示：(`StatusEffect`, `duration_ticks`, `amplifier`, `ambient`, `show_particles`, `show_icon`)
    pub effects: Mutex<Vec<EffectEntry>>,
    pub radius: Mutex<f32>,
    pub duration: Mutex<i32>,
    pub age: Mutex<i32>,
    /// 对同一实体重复应用之间的间隔刻数
    pub reapplication_delay: Mutex<i32>,
    /// `entity_id` -> 该实体再次可被影响前剩余刻数的映射
    pub reapplication_map: Mutex<HashMap<i32, i32>>,
    /// 每刻的线性半径变化
    pub radius_on_tick: Mutex<f32>,
    /// 半径会随实体受影响而变化
    pub radius_on_use: Mutex<f32>,
    /// 实体受影响时的持续时间变更（刻）
    pub duration_on_use: Mutex<i32>,
    /// 云生效并开始施加效果前的等待刻数（宽限期）
    pub wait_time: Mutex<i32>,
}

impl AreaEffectCloudEntity {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(entity: Entity) -> Arc<dyn EntityBase> {
        entity
            .no_physics
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let cloud = Self {
            entity,
            item_stack: Mutex::new(ItemStack::new(0, &papokin_data::item::Item::GLASS_BOTTLE)),
            effects: Mutex::new(Vec::new()),
            radius: Mutex::new(3.0),
            duration: Mutex::new(600), // 滞留药水的默认值
            age: Mutex::new(0),
            reapplication_delay: Mutex::new(20),
            reapplication_map: Mutex::new(HashMap::new()),
            radius_on_tick: Mutex::new(-3.0 / 600.0), // 滞留药水的默认值
            radius_on_use: Mutex::new(0.0),
            duration_on_use: Mutex::new(0),
            wait_time: Mutex::new(20),
        };

        Arc::new(cloud)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        entity: Entity,
        item_stack: ItemStack,
        effects_in: Vec<EffectEntry>,
        duration_in: i32,
        radius_in: f32,
        reapplication_delay_in: i32,
        wait_time_in: i32,
        radius_on_use_in: f32,
        duration_on_use_in: i32,
    ) -> Arc<dyn EntityBase> {
        entity
            .no_physics
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let cloud = Self {
            entity,
            item_stack: Mutex::new(item_stack),
            effects: Mutex::new(effects_in),
            radius: Mutex::new(radius_in),
            duration: Mutex::new(duration_in),
            age: Mutex::new(0),
            reapplication_delay: Mutex::new(reapplication_delay_in),
            reapplication_map: Mutex::new(HashMap::new()),
            radius_on_tick: Mutex::new(-radius_in / (duration_in as f32).max(1.0)),
            radius_on_use: Mutex::new(radius_on_use_in),
            duration_on_use: Mutex::new(duration_on_use_in),
            wait_time: Mutex::new(wait_time_in),
        };

        Arc::new(cloud)
    }
}

impl EntityBase for AreaEffectCloudEntity {
    fn init_data_tracker(&self) {
        // 发送初始半径和粒子（颜色），使客户端正确渲染
        let radius = *self
            .radius
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 计算粒子颜色
        let stack = self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let effects = self
            .effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        // 使用 ARGB 格式
        let mut color: i32 = (0xFFi32 << 24) | 0x385dc6; // 默认类水颜色

        if let Some(pc) =
            stack.get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
        {
            if let Some(c) = pc.custom_color {
                color = c | (0xFFi32 << 24);
            } else if !effects.is_empty() {
                let mut r_sum = 0.0f32;
                let mut g_sum = 0.0f32;
                let mut b_sum = 0.0f32;
                let count = effects.len() as f32;
                for (eff, _, _, _, _, _) in &effects {
                    let c = eff.color;
                    r_sum += ((c >> 16) & 0xFF) as f32;
                    g_sum += ((c >> 8) & 0xFF) as f32;
                    b_sum += (c & 0xFF) as f32;
                }
                let r = (r_sum / count) as i32;
                let g = (g_sum / count) as i32;
                let b = (b_sum / count) as i32;
                color = (0xFFi32 << 24) | (r << 16) | (g << 8) | b;
            }
        } else if !effects.is_empty() {
            let mut r_sum = 0.0f32;
            let mut g_sum = 0.0f32;
            let mut b_sum = 0.0f32;
            let count = effects.len() as f32;
            for (eff, _, _, _, _, _) in &effects {
                let c = eff.color;
                r_sum += ((c >> 16) & 0xFF) as f32;
                g_sum += ((c >> 8) & 0xFF) as f32;
                b_sum += (c & 0xFF) as f32;
            }
            let r = (r_sum / count) as i32;
            let g = (g_sum / count) as i32;
            let b = (b_sum / count) as i32;
            color = (0xFFi32 << 24) | (r << 16) | (g << 8) | b;
        }

        // 为 ENTITY_EFFECT 构建原始粒子选项字节
        let data_bytes = color.to_be_bytes();

        let meta = ParticleMeta {
            particle_id: papokin_protocol::codec::var_int::VarInt(
                papokin_data::particle::Particle::EntityEffect as i32,
            ),
            data: Box::new(data_bytes),
        };

        self.entity.set_synced_data(
            papokin_data::tracked_data::area_effect_cloud::PARTICLE,
            meta,
        );

        self.entity.set_synced_data(
            papokin_data::tracked_data::area_effect_cloud::RADIUS,
            radius,
        );

        // 初始等待标志
        let wait_time = *self
            .wait_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let is_waiting = 0 < wait_time;
        self.entity.set_synced_data(
            papokin_data::tracked_data::area_effect_cloud::WAITING,
            is_waiting,
        );
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::semicolon_outside_block)]
    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        // 年龄与持续时间处理
        {
            let mut age = self
                .age
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *age += 1;
            let duration = *self
                .duration
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *age > duration {
                // 移除旧实体
                self.entity.remove();
                return;
            }
        }

        // 获取当前龄期和等待时间
        let age = *self
            .age
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let wait_time = *self
            .wait_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 等待期结束时通知客户端，使其渲染完整粒子
        if age == wait_time && wait_time > 0 {
            self.entity.set_synced_data(
                papokin_data::tracked_data::area_effect_cloud::WAITING,
                false,
            );
        }

        if age < wait_time {
            // 遵守等待/宽限期
            return;
        }

        // 更新半径
        {
            let mut radius = self
                .radius
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let delta = *self
                .radius_on_tick
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *radius += delta;
            let current_radius = *radius;
            if current_radius <= 0.0 {
                self.entity.remove();
                return;
            }

            // 发送新半径
            drop(radius);
            self.entity.set_synced_data(
                papokin_data::tracked_data::area_effect_cloud::RADIUS,
                current_radius,
            );
        }

        // 每刻递减重复施加映射
        {
            let mut map = self
                .reapplication_map
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.retain(|_, v| {
                *v -= 1;
                *v > 0
            });
        }

        // 若符合条件，将效果应用于附近的实体
        let pos = self.entity.pos.load();
        let r = *self
            .radius
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) as f64;
        let min = Vector3::new(pos.x - r, pos.y - r, pos.z - r);
        let max = Vector3::new(pos.x + r, pos.y + r, pos.z + r);
        let aabb = BoundingBox::new(min, max);
        let world = self.entity.world.load();

        let mut candidates = world.get_entities_at_box(&aabb);
        let players = world.get_players_at_box(&aabb);
        for p in players {
            candidates.push(p.clone() as Arc<dyn EntityBase>);
        }

        // 本刻区域效果云即将影响的实体，先收集以便
        // apply 事件可覆盖整批（并将其取消）。
        let mut to_apply: Vec<(i32, Arc<dyn EntityBase>, f32)> = Vec::new();

        for cand in candidates {
            let cand_clone = cand.clone();

            // 跳过自身和其他 `AreaEffectCloud` 实体
            if cand_clone.get_entity().entity_id == self.get_entity().entity_id {
                continue;
            }
            if *cand_clone.get_entity().entity_type
                == papokin_data::entity::EntityType::AREA_EFFECT_CLOUD
            {
                continue;
            }

            // 尽早确定候选 id
            let ent_id = cand_clone.get_entity().entity_id;

            {
                let map = self
                    .reapplication_map
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if map.contains_key(&ent_id) {
                    continue;
                }
            }

            let radius_f = *self
                .radius
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                as f64;
            let pos_e = cand_clone.get_entity().pos.load();
            let dx = pos_e.x - pos.x;
            let dy = pos_e.y - pos.y;
            let dz = pos_e.z - pos.z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist > radius_f {
                continue;
            }

            let scale = 1.0f32 - (dist as f32 / radius_f as f32);

            // 判断此次接触是否真的会施加效果
            let effs_clone = self
                .effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let mut will_apply = false;

            // 只有生物实体才能获得效果
            if let Some(living_ref) = cand_clone.get_living_entity() {
                for (eff, _, _, _, _, _) in &effs_clone {
                    // 即时效果总是生效
                    let is_instant = eff.id
                        == papokin_data::effect::StatusEffect::INSTANT_DAMAGE.id
                        || eff.id == papokin_data::effect::StatusEffect::INSTANT_HEALTH.id;
                    if is_instant {
                        will_apply = true;
                        break;
                    }

                    // 仅当实体尚未拥有该效果时才应用
                    if !living_ref.has_effect(eff) {
                        will_apply = true;
                        break;
                    }
                }
            }

            // 如果没有任何可应用的内容，则跳过
            if !will_apply {
                continue;
            }

            to_apply.push((ent_id, cand_clone, scale));
        }

        if to_apply.is_empty() {
            return;
        }

        let affected_ids: Vec<i32> = to_apply.iter().map(|(id, _, _)| *id).collect();
        let mut apply_event =
            crate::plugin::api::events::entity::area_effect_cloud_apply::AreaEffectCloudApplyEvent::new(
                self.entity.entity_id,
                affected_ids,
            );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut apply_event);
        }
        if apply_event.cancelled {
            return;
        }

        for (ent_id, cand_clone, scale) in to_apply {
            // 在生成的任务内应用缩放后的效果
            let effs = self
                .effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(living) = cand_clone.get_living_entity() {
                crate::item::potion::PotionContents::apply_effects_to(
                    living,
                    effs,
                    scale,
                    crate::item::potion::PotionApplicationSource::AreaEffectCloud,
                );
            }

            // 为该实体设置重新施加的延迟
            let delay = *self
                .reapplication_delay
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.reapplication_map
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(ent_id, delay);

            // 应用使用时缩小半径（收缩）
            let radius_on_use = *self
                .radius_on_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if radius_on_use != 0.0 {
                let mut radius_lock = self
                    .radius
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *radius_lock += radius_on_use;
                let current_radius = *radius_lock;
                if current_radius < 0.5 {
                    drop(radius_lock);
                    self.entity.remove();
                    return;
                }
                drop(radius_lock);

                // 将更新后的半径发送给客户端
                self.entity.set_synced_data(
                    papokin_data::tracked_data::area_effect_cloud::RADIUS,
                    current_radius,
                );
            }

            // 应用使用时缩短时长（缩短存在时间）
            let duration_on_use = *self
                .duration_on_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if duration_on_use != 0 {
                let mut duration_lock = self
                    .duration
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if *duration_lock != -1 {
                    *duration_lock += duration_on_use;
                    if *duration_lock <= 0 {
                        drop(duration_lock);
                        self.entity.remove();
                        return;
                    }
                }
            }
        }
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
}
