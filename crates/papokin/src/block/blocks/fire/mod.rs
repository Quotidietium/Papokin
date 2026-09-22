use std::sync::Arc;

use papokin_data::dimension::Dimension;
use papokin_data::tag::Taggable;
use papokin_data::world::WorldEvent;
use papokin_data::{Block, BlockDirection, tag};
use papokin_util::math::position::BlockPos;
use papokin_util::random::RandomGenerator;
use papokin_util::random::xoroshiro128::Xoroshiro;
use rand::RngExt;
use soul_fire::SoulFireBlock;

use crate::block::blocks::fire::fire::FireBlock;
use crate::block::{BlockBehaviour, CanPlaceAtArgs, OnEntityCollisionArgs};
use crate::entity::EntityBase;
use crate::world::World;
use crate::world::portal::nether::NetherPortal;
use papokin_data::damage::DamageType;
use papokin_data::entity::EntityType;
use std::sync::atomic::Ordering;

#[expect(clippy::module_inception)]
pub mod fire;
pub mod soul_fire;

pub struct FireBlockBase;

impl FireBlockBase {
    pub fn get_fire_type(world: &World, pos: &BlockPos) -> Block {
        let block = world.get_block(&pos.down());
        if SoulFireBlock::is_soul_base(block) {
            return Block::SOUL_FIRE;
        }
        Block::FIRE
    }

    #[must_use]
    pub fn can_place_on(block: &Block) -> bool {
        // 确保下方方块不是火焰方块或流体方块
        block != &Block::SOUL_FIRE
            && block != &Block::FIRE
            && block != &Block::WATER
            && block != &Block::LAVA
    }

    pub fn is_soul_fire(world: &Arc<World>, block_pos: &BlockPos) -> bool {
        let block = world.get_block(&block_pos.down());
        block.has_tag(&tag::Block::MINECRAFT_SOUL_FIRE_BASE_BLOCKS)
    }

    pub fn can_place_at(world: &Arc<World>, block_pos: &BlockPos) -> bool {
        let block_state = world.get_block_state(block_pos);
        if !block_state.is_air() {
            return false;
        }
        if Self::is_soul_fire(world, block_pos) {
            SoulFireBlock.can_place_at(CanPlaceAtArgs {
                server: None,
                world: Some(world),
                block_accessor: world.as_ref(),
                block: &Block::SOUL_FIRE,
                state: Block::SOUL_FIRE.default_state,
                position: block_pos,
                direction: None,
                player: None,
                use_item_on: None,
            })
        } else {
            FireBlock.can_place_at(CanPlaceAtArgs {
                server: None,
                world: Some(world),
                block_accessor: world.as_ref(),
                block: &Block::FIRE,
                state: Block::FIRE.default_state,
                position: block_pos,
                direction: None,
                player: None,
                use_item_on: None,
            }) || Self::should_light_portal_at(world, block_pos, BlockDirection::Up)
        }
    }

    pub fn should_light_portal_at(
        world: &Arc<World>,
        block_pos: &BlockPos,
        direction: BlockDirection,
    ) -> bool {
        let dimension = &world.dimension;
        if dimension != &Dimension::OVERWORLD && dimension != &Dimension::THE_NETHER {
            return false;
        }
        let mut found = false;

        for dir in BlockDirection::all() {
            if world.get_block(&block_pos.offset(dir.to_offset())) == &Block::OBSIDIAN {
                found = true;
                break;
            }
        }

        if !found {
            return false;
        }

        let dir = if direction.is_horizontal() {
            direction
                .rotate_counter_clockwise()
                .to_horizontal_axis()
                .unwrap_or(papokin_data::block_properties::HorizontalAxis::X)
        } else {
            BlockDirection::random_horizontal(&mut RandomGenerator::Xoroshiro(
                Xoroshiro::from_seed(rand::rng().random()),
            ))
            .to_axis()
        };
        NetherPortal::get_new_portal(world, block_pos, dir).is_some()
    }

    /// `fire` 与 `soul_fire` 共用的火焰碰撞行为。
    pub fn apply_fire_collision(args: &OnEntityCollisionArgs<'_>, extra_damage_for_living: bool) {
        let base_entity = args.entity.get_entity();
        if !base_entity.entity_type.fire_immune && !base_entity.fire_immune.load(Ordering::Relaxed)
        {
            let ticks = base_entity.fire_ticks.load(Ordering::Relaxed);

            // 计时器逻辑
            if ticks < 0 {
                base_entity.fire_ticks.store(ticks + 1, Ordering::Relaxed);
            } else if base_entity.entity_type == &EntityType::PLAYER {
                let rnd_ticks = rand::rng().random_range(1..3);
                base_entity
                    .fire_ticks
                    .store(ticks + rnd_ticks, Ordering::Relaxed);
            }

            // 施加火焰刻
            if base_entity.fire_ticks.load(Ordering::Relaxed) >= 0 {
                let mut combust_event =
                    crate::plugin::api::events::entity::entity_combust_by_block::EntityCombustByBlockEvent::new(
                        base_entity.entity_id,
                        *args.position,
                        8.0,
                    );
                if let Some(server) = args.world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut combust_event);
                }
                if !combust_event.cancelled {
                    args.entity.set_on_fire_for(combust_event.duration);
                }
            }

            // 普通火焰与灵魂火焰的伤害
            if extra_damage_for_living {
                base_entity.damage(args.entity, 2.0, DamageType::IN_FIRE);
            } else {
                base_entity.damage(args.entity, 1.0, DamageType::IN_FIRE);
            }
        }
    }

    fn broken(world: &World, block_pos: BlockPos) {
        world.sync_world_event(WorldEvent::SoundExtinguishFire, block_pos, 0);
    }
}
