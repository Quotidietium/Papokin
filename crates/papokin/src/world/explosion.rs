use std::sync::Arc;

use papokin_data::{
    Block, BlockState, BlockStateId,
    damage::DamageType,
    entity::EntityType,
    fluid::Fluid,
    tag::{Tag, Taggable},
};
use papokin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use papokin_world::chunk::ChunkData;
use rustc_hash::FxHashMap;

use crate::{
    block::{ExplodeArgs, drop_loot},
    entity::{Entity, EntityBase},
    world::loot::LootContextParameters,
};

use super::{BlockFlags, World};

/// 定义爆炸与世界交互的类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplosionInteraction {
    None,
    Block,
    Mob,
    Tnt,
    Trigger,
}

/// 定义爆炸如何与世界中的方块交互。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockInteraction {
    /// 保持方块完好（不破坏方块、不掉落物品）。
    Keep,
    /// 摧毁方块并 100% 掉落物品，无衰减。
    Destroy,
    /// 摧毁方块并根据爆炸半径应用战利品衰减。
    DestroyWithDecay,
    /// 触发方块效果而不破坏方块。
    TriggerBlock,
}

/// 定义如何为爆炸计算伤害和方块破坏。
pub trait ExplosionDamageCalculator: Send + Sync {
    ///返回该方块的爆炸抗性。若为 None，则该方块被视为空气/空。
    fn get_block_explosion_resistance(
        &self,
        _explosion: &Explosion,
        _world: &World,
        _pos: &BlockPos,
        block: &Block,
        fluid: &papokin_data::fluid::FluidState,
    ) -> Option<f32> {
        if block.default_state.is_air() && fluid.is_empty {
            None
        } else {
            Some(fluid.blast_resistance.max(block.blast_resistance))
        }
    }

    /// 返回此方块是否应被爆炸摧毁/受爆炸影响。
    fn should_block_explode(
        &self,
        _explosion: &Explosion,
        _world: &World,
        _pos: &BlockPos,
        _block: &Block,
        _power: f32,
    ) -> bool {
        true
    }

    /// 返回实体是否应受到爆炸伤害。
    fn should_damage_entity(&self, _explosion: &Explosion, _entity: &dyn EntityBase) -> bool {
        true
    }

    ///返回给定实体的击退倍率（默认 1.0）。
    fn get_knockback_multiplier(&self, _entity: &dyn EntityBase) -> f32 {
        1.0
    }

    /// 根据暴露程度计算要对实体造成的伤害值。
    fn get_entity_damage_amount(
        &self,
        explosion: &Explosion,
        entity: &dyn EntityBase,
        exposure: f32,
    ) -> f32 {
        let radius = explosion.power as f64 * 2.0;
        let distance = (entity
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&explosion.pos))
        .sqrt()
            / radius;
        let damage_multiplier = (1.0 - distance) * exposure as f64;
        (f64::midpoint(damage_multiplier * damage_multiplier, damage_multiplier)
            * 7.0
            * explosion.power as f64
            + 1.0) as f32
    }
}

/// 默认爆炸伤害计算器，实现原版标准爆炸规则。
pub struct DefaultExplosionDamageCalculator;

impl ExplosionDamageCalculator for DefaultExplosionDamageCalculator {}

/// 一个可配置的爆炸伤害计算器（例如用于风弹、重锤的风爆）。
pub struct SimpleExplosionDamageCalculator {
    pub damages_entities: bool,
    pub damages_blocks: bool,
    pub knockback_multiplier: Option<f32>,
    pub immune_blocks: Option<&'static Tag>,
}

impl SimpleExplosionDamageCalculator {
    #[must_use]
    pub const fn new(
        damages_entities: bool,
        damages_blocks: bool,
        knockback_multiplier: Option<f32>,
        immune_blocks: Option<&'static Tag>,
    ) -> Self {
        Self {
            damages_entities,
            damages_blocks,
            knockback_multiplier,
            immune_blocks,
        }
    }
}

impl ExplosionDamageCalculator for SimpleExplosionDamageCalculator {
    fn get_block_explosion_resistance(
        &self,
        _explosion: &Explosion,
        _world: &World,
        _pos: &BlockPos,
        block: &Block,
        fluid: &papokin_data::fluid::FluidState,
    ) -> Option<f32> {
        if let Some(immune_tag) = self.immune_blocks
            && block.has_tag(immune_tag)
        {
            return None;
        }
        if block.default_state.is_air() && fluid.is_empty {
            None
        } else {
            Some(fluid.blast_resistance.max(block.blast_resistance))
        }
    }

    fn should_block_explode(
        &self,
        _explosion: &Explosion,
        _world: &World,
        _pos: &BlockPos,
        block: &Block,
        _power: f32,
    ) -> bool {
        if !self.damages_blocks {
            return false;
        }
        if let Some(immune_tag) = self.immune_blocks
            && block.has_tag(immune_tag)
        {
            return false;
        }
        true
    }

    fn should_damage_entity(&self, _explosion: &Explosion, _entity: &dyn EntityBase) -> bool {
        self.damages_entities
    }

    fn get_knockback_multiplier(&self, _entity: &dyn EntityBase) -> f32 {
        self.knockback_multiplier.unwrap_or(1.0)
    }
}

pub struct Explosion {
    power: f32,
    pos: Vector3<f64>,
    block_interaction: BlockInteraction,
    damage_calculator: Option<Arc<dyn ExplosionDamageCalculator>>,
    preserve_rails: bool,
}

impl Explosion {
    /// 爆炸威力的安全上限。原版内容最高为 6（末地水晶）；每条
    /// 破坏射线的步进数与威力成正比，未经钳制的威力（如外部
    /// 编辑存档注入的火球 `ExplosionPower`）会把单次爆炸的射线
    /// 步进数放大到亿级，卡死 tick 线程。
    pub const MAX_POWER: f32 = 64.0;

    #[must_use]
    pub const fn new(power: f32, pos: Vector3<f64>, block_interaction: BlockInteraction) -> Self {
        // 非有限值按 0 处理（无破坏），超界值钳制到安全上限
        let power = if power.is_nan() {
            0.0
        } else {
            power.clamp(0.0, Self::MAX_POWER)
        };
        Self {
            power,
            pos,
            block_interaction,
            damage_calculator: None,
            preserve_rails: false,
        }
    }

    #[must_use]
    pub fn with_damage_calculator(
        mut self,
        calculator: Arc<dyn ExplosionDamageCalculator>,
    ) -> Self {
        self.damage_calculator = Some(calculator);
        self
    }

    #[must_use]
    pub const fn preserving_rails(mut self) -> Self {
        self.preserve_rails = true;
        self
    }

    /// 钳制后的爆炸威力（见 [`Self::MAX_POWER`]）。
    #[must_use]
    pub const fn power(&self) -> f32 {
        self.power
    }

    fn protects_rail(&self, world: &World, pos: &BlockPos, block: &Block) -> bool {
        self.preserve_rails && (Self::is_rail(block) || Self::is_rail(world.get_block(&pos.up())))
    }

    fn is_rail(block: &Block) -> bool {
        block.id == Block::RAIL.id
            || block.id == Block::POWERED_RAIL.id
            || block.id == Block::DETECTOR_RAIL.id
            || block.id == Block::ACTIVATOR_RAIL.id
    }

    #[allow(clippy::too_many_lines)]
    fn get_blocks_to_destroy(
        &self,
        world: &World,
    ) -> FxHashMap<BlockPos, (&'static Block, &'static BlockState)> {
        let mut map = FxHashMap::default();

        let mut chunk_cache: FxHashMap<
            papokin_util::math::vector2::Vector2<i32>,
            Option<Arc<ChunkData>>,
        > = FxHashMap::default();

        let default_calc = DefaultExplosionDamageCalculator;
        let calc: &dyn ExplosionDamageCalculator = match &self.damage_calculator {
            Some(c) => c.as_ref(),
            None => &default_calc,
        };

        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    if x > 0 && x < 15 && y > 0 && y < 15 && z > 0 && z < 15 {
                        continue;
                    }

                    let mut dir_x = f64::from(x) / 7.5 - 1.0;
                    let mut dir_y = f64::from(y) / 7.5 - 1.0;
                    let mut dir_z = f64::from(z) / 7.5 - 1.0;

                    let length = (dir_x * dir_x + dir_y * dir_y + dir_z * dir_z).sqrt();
                    dir_x /= length;
                    dir_y /= length;
                    dir_z /= length;

                    let mut pos_x = self.pos.x;
                    let mut pos_y = self.pos.y;
                    let mut pos_z = self.pos.z;

                    let random_val = rand::random::<f32>();
                    let mut h = self.power * random_val.mul_add(0.6, 0.7);

                    while h > 0.0 {
                        let block_pos = BlockPos::floored(pos_x, pos_y, pos_z);

                        if !world.is_in_build_limit(block_pos) {
                            break;
                        }

                        let (chunk_pos, relative) = block_pos.chunk_and_chunk_relative_position();

                        let chunk_opt = chunk_cache.entry(chunk_pos).or_insert_with(|| {
                            world
                                .level
                                .read_chunk_sync(&chunk_pos, std::clone::Clone::clone)
                        });

                        let state_id = if let Some(chunk) = chunk_opt {
                            chunk
                                .section
                                .get_block_absolute_y(
                                    relative.x as usize,
                                    block_pos.0.y,
                                    relative.z as usize,
                                )
                                .unwrap_or(Block::AIR.default_state.id)
                        } else {
                            Block::AIR.default_state.id
                        };

                        let (block, state) = BlockState::from_id_with_block(state_id);

                        let (_fluid, fluid_state) = Fluid::from_state_id(state_id).map_or_else(
                            || {
                                if block.is_waterlogged(state_id) {
                                    (&Fluid::FLOWING_WATER, &Fluid::FLOWING_WATER.states[0])
                                } else {
                                    (&Fluid::EMPTY, &Fluid::EMPTY.states[0])
                                }
                            },
                            |raw_fluid| {
                                let f = raw_fluid.to_flowing();
                                (f, &f.states[0])
                            },
                        );

                        if !state.is_air() || !fluid_state.is_empty {
                            let protects_rail = self.protects_rail(world, &block_pos, block);
                            let resistance = if protects_rail {
                                Some(0.0)
                            } else {
                                calc.get_block_explosion_resistance(
                                    self,
                                    world,
                                    &block_pos,
                                    block,
                                    fluid_state,
                                )
                            };

                            if let Some(resistance) = resistance {
                                h -= (resistance + 0.3) * 0.3;
                            }

                            if h > 0.0
                                && !protects_rail
                                && calc.should_block_explode(self, world, &block_pos, block, h)
                            {
                                map.insert(block_pos, (block, state));
                            }
                        }

                        pos_x += dir_x * 0.3;
                        pos_y += dir_y * 0.3;
                        pos_z += dir_z * 0.3;
                        h -= 0.225_000_01;
                    }
                }
            }
        }
        map
    }

    fn damage_entities(&self, world: &Arc<World>) {
        // 爆炸太小
        if self.power < 1.0e-5 {
            return;
        }

        let radius = self.power as f64 * 2.0;
        let min_x = (self.pos.x - radius - 1.0).floor() as i32;
        let max_x = (self.pos.x + radius + 1.0).floor() as i32;
        let min_y = (self.pos.y - radius - 1.0).floor() as i32;
        let max_y = (self.pos.y + radius + 1.0).floor() as i32;
        let min_z = (self.pos.z - radius - 1.0).floor() as i32;
        let max_z = (self.pos.z + radius + 1.0).floor() as i32;

        let search_box = BoundingBox::new(
            Vector3::new(min_x as f64, min_y as f64, min_z as f64),
            Vector3::new(max_x as f64, max_y as f64, max_z as f64),
        );

        let entities = world.get_all_at_box(&search_box);

        let default_calc = DefaultExplosionDamageCalculator;
        let calc: &dyn ExplosionDamageCalculator = match &self.damage_calculator {
            Some(c) => c.as_ref(),
            None => &default_calc,
        };

        for entity_base in entities {
            if entity_base.is_immune_to_explosion() {
                continue;
            }

            // 跳过旁观玩家（无伤害、无击退）
            if entity_base.is_spectator() {
                continue;
            }

            let entity = entity_base.get_entity();

            let distance = (entity.pos.load().squared_distance_to_vec(&self.pos)).sqrt() / radius;
            if distance > 1.0 {
                continue;
            }

            let should_damage = calc.should_damage_entity(self, entity_base.as_ref());
            let knockback_multiplier = calc.get_knockback_multiplier(entity_base.as_ref()) as f64;

            let exposure = if !should_damage && knockback_multiplier == 0.0 {
                0.0
            } else {
                Self::calculate_exposure(&self.pos, entity, world) as f64
            };

            if exposure == 0.0 {
                continue;
            }

            if should_damage {
                let damage =
                    calc.get_entity_damage_amount(self, entity_base.as_ref(), exposure as f32);
                entity.damage(entity_base.as_ref(), damage, DamageType::EXPLOSION);
            }

            // 计算并应用击退
            let dir_pos = if entity.entity_type == &EntityType::TNT {
                entity.pos.load()
            } else {
                entity.get_eye_pos()
            };
            let direction = (dir_pos - self.pos).normalize();
            // TODO: 实体爆炸击退抗性属性
            let knockback_resistance = 0.0;

            let knockback_power =
                (1.0 - distance) * exposure * knockback_multiplier * (1.0 - knockback_resistance);
            let knockback = direction * knockback_power;
            entity.add_velocity(knockback);
        }
    }

    fn calculate_exposure(
        explosion_pos: &Vector3<f64>,
        entity: &Entity,
        world: &Arc<World>,
    ) -> f32 {
        let bbox = entity.bounding_box.load();

        let step_x = 1.0 / ((bbox.max.x - bbox.min.x) * 2.0 + 1.0);
        let step_y = 1.0 / ((bbox.max.y - bbox.min.y) * 2.0 + 1.0);
        let step_z = 1.0 / ((bbox.max.z - bbox.min.z) * 2.0 + 1.0);

        if step_x < 0.0 || step_y < 0.0 || step_z < 0.0 {
            return 0.0;
        }

        let offset_x = (1.0 - (1.0 / step_x).floor() * step_x) / 2.0;
        let offset_z = (1.0 - (1.0 / step_z).floor() * step_z) / 2.0;

        let mut visible_points = 0;
        let mut total_points = 0;

        let mut k = 0.0;
        while k <= 1.0 {
            let mut l = 0.0;
            while l <= 1.0 {
                let mut m = 0.0;
                while m <= 1.0 {
                    let n = bbox.min.x + (bbox.max.x - bbox.min.x) * k;
                    let o = bbox.min.y + (bbox.max.y - bbox.min.y) * l;
                    let p = bbox.min.z + (bbox.max.z - bbox.min.z) * m;

                    let vec3d = Vector3::new(n + offset_x, o, p + offset_z);

                    if world
                        .raycast(vec3d, *explosion_pos, |pos, world_ref| {
                            let state = world_ref.get_block_state(pos);
                            !state.is_air() && !state.collision_shapes.is_empty()
                        })
                        .is_none()
                    {
                        visible_points += 1;
                    }

                    total_points += 1;
                    m += step_z;
                }
                l += step_y;
            }
            k += step_x;
        }

        if total_points == 0 {
            return 0.0;
        }

        visible_points as f32 / total_points as f32
    }

    /// 返回被移除的方块数量
    pub fn explode(&self, world: &Arc<World>) -> u32 {
        self.damage_entities(world);

        match self.block_interaction {
            BlockInteraction::Keep => 0,
            BlockInteraction::TriggerBlock => {
                let blocks = self.get_blocks_to_destroy(world);
                for (pos, (block, _state)) in &blocks {
                    let pumpkin_block = world.block_registry.get_pumpkin_block(block.id);
                    if let Some(pumpkin_block) = pumpkin_block {
                        pumpkin_block.explode(ExplodeArgs {
                            world,
                            block,
                            position: pos,
                        });
                    }
                }
                0
            }
            BlockInteraction::Destroy | BlockInteraction::DestroyWithDecay => {
                let center_pos = BlockPos::floored(self.pos.x, self.pos.y, self.pos.z);
                let mut event =
                    crate::plugin::api::events::block::block_explode::BlockExplodeEvent::new(
                        center_pos,
                        if self.power > 0.0 {
                            1.0 / self.power
                        } else {
                            1.0
                        },
                    );
                if let Some(server) = world.server.upgrade() {
                    server.plugin_manager.fire_blocking(&server, &mut event);
                }
                if event.cancelled {
                    return 0;
                }

                let blocks = self.get_blocks_to_destroy(world);
                let decay_drops = self.block_interaction == BlockInteraction::DestroyWithDecay;
                let explosion_radius = decay_drops.then_some(self.power);

                for (pos, (block, state)) in &blocks {
                    let pumpkin_block = world.block_registry.get_pumpkin_block(block.id);

                    if pumpkin_block.is_none_or(|s| s.should_drop_items_on_explosion()) {
                        let is_raining = world.is_raining();
                        let is_thundering = world.is_thundering();
                        let params = LootContextParameters {
                            block_state: Some(state),
                            explosion_radius,
                            position: Some(papokin_util::math::vector3::Vector3::new(
                                pos.0.x as f64,
                                pos.0.y as f64,
                                pos.0.z as f64,
                            )),
                            world_time: world.level_info.load().day_time as u64,
                            is_raining: Some(is_raining),
                            is_thundering: Some(is_thundering),
                            ..Default::default()
                        };
                        drop_loot(world, block, pos, false, &params);
                    }

                    world.set_block_state(pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
                    world.close_container_screens_at(pos);

                    if let Some(pumpkin_block) = pumpkin_block {
                        pumpkin_block.explode(ExplodeArgs {
                            world,
                            block,
                            position: pos,
                        });
                    }
                }
                // TODO: 火
                blocks.len() as u32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Explosion;
    use papokin_data::Block;

    #[test]
    fn tnt_minecart_rail_protection_covers_every_rail_type() {
        for rail in [
            &Block::RAIL,
            &Block::POWERED_RAIL,
            &Block::DETECTOR_RAIL,
            &Block::ACTIVATOR_RAIL,
        ] {
            assert!(Explosion::is_rail(rail));
        }
        assert!(!Explosion::is_rail(&Block::STONE));
    }
}
