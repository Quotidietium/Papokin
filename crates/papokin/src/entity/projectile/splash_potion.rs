use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use crate::{
    entity::{Entity, EntityBase, projectile::ThrownItemEntity},
    server::Server,
};
use papokin_data::item_stack::ItemStack;
use papokin_data::{Block, BlockId};
use papokin_protocol::java::client::play::CWorldEvent;
use papokin_util::math::vector3::Vector3;
use papokin_util::math::{boundingbox::BoundingBox, vector2::Vector2};
use papokin_util::math::{position::BlockPos, vector2::to_chunk_pos};
use papokin_world::world::BlockFlags;

const GRAVITY: f64 = 0.05;

pub struct SplashPotionEntity {
    pub thrown: ThrownItemEntity,
    pub item_stack: RwLock<ItemStack>,
}

impl SplashPotionEntity {
    pub fn new(entity: Entity) -> Self {
        entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));
        let thrown = ThrownItemEntity {
            entity,
            owner_id: None,
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            gravity: GRAVITY,
        };

        Self {
            thrown,
            item_stack: RwLock::new(ItemStack::new(1, &papokin_data::item::Item::SPLASH_POTION)),
        }
    }

    pub fn new_shot(entity: Entity, shooter: &Entity) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        thrown.entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));
        Self {
            thrown,
            item_stack: RwLock::new(ItemStack::new(1, &papokin_data::item::Item::SPLASH_POTION)),
        }
    }

    pub fn set_item_stack(&self, item_stack: ItemStack) {
        let mut write = self
            .item_stack
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *write = item_stack;
    }
}

fn is_water_potion(stack: &ItemStack) -> bool {
    stack
        .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
        .and_then(|pc| pc.potion_id)
        == Some(papokin_data::potion::Potion::WATER.id as i32)
}

/// 熄灭命中位置及其四个水平相邻位置上的火（包括灵魂火）。
fn extinguish_fire(world: &Arc<crate::world::World>, hit_pos: Vector3<f64>) {
    let air_state_id = Block::AIR.default_state.id;

    let neighbors = [
        hit_pos,
        Vector3::new(hit_pos.x + 1.0, hit_pos.y, hit_pos.z),
        Vector3::new(hit_pos.x - 1.0, hit_pos.y, hit_pos.z),
        Vector3::new(hit_pos.x, hit_pos.y, hit_pos.z + 1.0),
        Vector3::new(hit_pos.x, hit_pos.y, hit_pos.z - 1.0),
    ];

    for p in neighbors {
        let pos = BlockPos(Vector3::new(
            p.x.floor() as i32,
            p.y.floor() as i32,
            p.z.floor() as i32,
        ));
        let state_id = world.get_block_state_id(&pos);
        let raw_block_id = state_id.to_block_id();
        if raw_block_id == BlockId::FIRE || raw_block_id == BlockId::SOUL_FIRE {
            world.set_block_state(&pos, air_state_id, BlockFlags::NOTIFY_ALL);
        }
    }
}

pub(crate) fn extinguish_fire_if_water_potion(
    world: &Arc<crate::world::World>,
    hit_pos: Vector3<f64>,
    stack: &ItemStack,
) {
    if is_water_potion(stack) {
        extinguish_fire(world, hit_pos);
    }
}

impl EntityBase for SplashPotionEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.thrown.owner_id
    }

    fn init_data_tracker(&self) {
        let entity = self.get_entity();
        let stack = self
            .item_stack
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 同步物品堆
        entity.set_synced_data(
            papokin_data::tracked_data::splash_potion::ITEM_STACK,
            papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer::from(stack.clone()),
        );
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.thrown.process_tick(caller);
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    #[allow(clippy::too_many_lines)]
    fn on_hit(&self, hit: crate::entity::projectile::ProjectileHit) {
        let world = self.get_entity().world.load();
        let hit_pos = hit.hit_pos();

        // 若是水药水则熄灭火焰
        let stack = self
            .item_stack
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        extinguish_fire_if_water_potion(&world, hit_pos, &stack);

        // 发送撞击实体状态（播放破坏粒子）
        world.send_entity_status(self.get_entity(), papokin_data::entity::EntityStatus::Death);

        let effects = crate::item::potion::PotionContents::read_potion_effects(&stack);

        // 计算颜色：若存在 custom_color 则使用之，否则混合各效果，再否则使用默认水色
        let mut color = 0x385dc6; // 默认类水颜色
        if let Some(pc) =
            stack.get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
        {
            if let Some(c) = pc.custom_color {
                color = c;
            } else if !effects.is_empty() {
                let mut r_sum = 0.0;
                let mut g_sum = 0.0;
                let mut b_sum = 0.0;
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
                color = (r << 16) | (g << 8) | b;
            }
        } else if !effects.is_empty() {
            let mut r_sum = 0.0;
            let mut g_sum = 0.0;
            let mut b_sum = 0.0;
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
            color = (r << 16) | (g << 8) | b;
        }

        // 播放水花粒子
        let has_instant = effects.iter().any(|(e, _, _, _, _, _)| {
            e.id == papokin_data::effect::StatusEffect::INSTANT_DAMAGE.id
                || e.id == papokin_data::effect::StatusEffect::INSTANT_HEALTH.id
        });
        let event_id = if has_instant { 2007 } else { 2002 };

        // 将 hit_pos 转换为 BlockPos
        let block_pos = BlockPos(Vector3::new(
            hit_pos.x.floor() as i32,
            hit_pos.y.floor() as i32,
            hit_pos.z.floor() as i32,
        ));
        world.broadcast_to_chunk(
            to_chunk_pos(&Vector2::new(block_pos.0.x, block_pos.0.z)),
            &CWorldEvent::new(event_id, block_pos, color, false),
        );

        // 如果没有任何效果，则只是泼溅（如同水瓶）
        if effects.is_empty() {
            // 水瓶喷溅：收集范围内的存活实体并
            // 通知插件（可取消）。
            let radius = 4.0f64;
            let min = Vector3::new(hit_pos.x - radius, hit_pos.y - radius, hit_pos.z - radius);
            let max = Vector3::new(hit_pos.x + radius, hit_pos.y + radius, hit_pos.z + radius);
            let aabb = BoundingBox::new(min, max);

            let mut candidates = world.get_entities_at_box(&aabb);
            let players = world.get_players_at_box(&aabb);
            for p in players {
                candidates.push(p.clone() as Arc<dyn EntityBase>);
            }

            let affected_ids: Vec<i32> = candidates
                .iter()
                .filter(|cand| cand.get_living_entity().is_some())
                .filter(|cand| {
                    let pos = cand.get_entity().pos.load();
                    let dx = pos.x - hit_pos.x;
                    let dy = pos.y - hit_pos.y;
                    let dz = pos.z - hit_pos.z;
                    (dx * dx + dy * dy + dz * dz).sqrt() <= radius
                })
                .map(|cand| cand.get_entity().entity_id)
                .collect();

            let mut event =
                crate::plugin::api::events::entity::water_bottle_splash::WaterBottleSplashEvent::new(
                    self.get_entity().entity_id,
                    affected_ids,
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            // 取消目前没有进一步可抑制的效果（水
            // 对水生敏感生物的伤害尚未实现），但
            // 标志仍会被读回以备将来使用。
            return;
        }

        let radius = 4.0f64;
        let min = Vector3::new(hit_pos.x - radius, hit_pos.y - radius, hit_pos.z - radius);
        let max = Vector3::new(hit_pos.x + radius, hit_pos.y + radius, hit_pos.z + radius);
        let aabb = BoundingBox::new(min, max);

        // 收集实体和玩家候选
        let mut candidates = world.get_entities_at_box(&aabb);
        let players = world.get_players_at_box(&aabb);
        for p in players {
            candidates.push(p.clone() as Arc<dyn EntityBase>);
        }

        let mut affected: Vec<(Arc<dyn EntityBase>, f32)> = Vec::new();
        for cand in candidates {
            if cand.get_living_entity().is_some() {
                let pos = cand.get_entity().pos.load();
                let dx = pos.x - hit_pos.x;
                let dy = pos.y - hit_pos.y;
                let dz = pos.z - hit_pos.z;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                if dist <= radius {
                    let scale = (1.0f32 - (dist as f32 / radius as f32)).max(0.0);
                    affected.push((cand, scale));
                }
            }
        }

        let affected_ids: Vec<i32> = affected
            .iter()
            .map(|(c, _)| c.get_entity().entity_id)
            .collect();
        if let Some(server) = world.server.upgrade() {
            let mut event =
                crate::plugin::api::events::entity::potion_splash::PotionSplashEvent::new(
                    self.get_entity().entity_id,
                    block_pos,
                    stack.item.registry_key.to_string(),
                    affected_ids,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
            affected.retain(|(c, _)| event.affected_entities.contains(&c.get_entity().entity_id));
        }

        for (cand, scale) in affected {
            if let Some(living) = cand.get_living_entity() {
                crate::item::potion::PotionContents::apply_effects_to(
                    living,
                    effects.clone(),
                    scale,
                    crate::item::potion::PotionApplicationSource::Normal,
                );
            }
        }
    }
}
