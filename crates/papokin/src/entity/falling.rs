use papokin_data::Block;
use papokin_data::BlockStateId;
use papokin_data::damage::DamageType;
use papokin_data::entity::EntityType;
use papokin_data::tag::{self, Taggable};
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockFlags;
use std::sync::{Arc, atomic::Ordering};

use crate::{
    block::blocks::falling::FallingBlock,
    entity::{Entity, EntityBase, living::LivingEntity},
    server::Server,
    world::World,
};

pub struct FallingEntity {
    entity: Entity,
    block_state_id: BlockStateId,
}

impl FallingEntity {
    pub const fn new(entity: Entity, block_state_id: BlockStateId) -> Self {
        Self {
            entity,
            block_state_id,
        }
    }

    /// 替换当前方块并生成一个新的下落方块（同步）
    pub fn replace_spawn(world: &Arc<World>, position: BlockPos, block_state: BlockStateId) {
        // 替换原方块，TODO: 使用流体状态
        world.set_block_state(
            &position,
            Block::AIR.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );

        let position = position.0.to_f64().add_raw(0.5, 0.0, 0.5);
        let entity = Entity::new(world.clone(), position, &EntityType::FALLING_BLOCK);
        entity
            .data
            .store(i32::from(block_state.as_u16()), Ordering::Relaxed);
        let entity = Arc::new(Self::new(entity, block_state));
        world.spawn_entity_non_save(entity);
    }
}

impl EntityBase for FallingEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;
        let mut velo = entity.velocity.load();
        velo.y -= self.get_gravity();

        entity.velocity.store(velo);

        entity.move_entity(caller, velo);
        entity.tick_block_collisions(caller);
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
            let world = entity.world.load();
            let landing_pos = self.entity.block_pos.load();
            let mut state_id = self.block_state_id;
            let block = Block::from_state_id(state_id);
            if block.has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS)
                && FallingBlock::should_solidify(&**world, &landing_pos)
                && let Some(name) = block.name.strip_suffix("_powder")
                && let Some(concrete) = Block::from_name(name)
            {
                state_id = concrete.default_state.id;
            }
            world.set_block_state(&landing_pos, state_id, BlockFlags::NOTIFY_ALL);
            self.entity.remove();
        }

        entity.velocity.store(velo.multiply(0.98, 0.98, 0.98));

        if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            entity.send_pos_rot();
            entity.send_velocity();
        }
    }

    fn init_data_tracker(&self) {
        self.entity.set_synced_data(
            papokin_data::tracked_data::falling_block::START_POS,
            self.entity.block_pos.load(),
        );
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }
    fn damage(&self, _caller: &dyn EntityBase, _amount: f32, _damage_type: DamageType) -> bool {
        false
    }

    fn get_gravity(&self) -> f64 {
        0.04
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
