use super::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use core::f32;
use papokin_data::Block;
use papokin_protocol::codec::var_int::VarInt;
use papokin_util::math::vector3::Vector3;
use std::{
    f64::consts::TAU,
    sync::atomic::{
        AtomicU32,
        Ordering::{self, Relaxed},
    },
};

pub struct TNTEntity {
    entity: Entity,
    power: f32,
    fuse: AtomicU32,
}

impl TNTEntity {
    pub const fn new(entity: Entity, power: f32, fuse: u32) -> Self {
        Self {
            entity,
            power,
            fuse: AtomicU32::new(fuse),
        }
    }
}

impl EntityBase for TNTEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;

        let mut velo = entity.velocity.load();
        velo.y -= self.get_gravity();

        entity.move_entity(caller, velo);
        entity.tick_block_collisions(caller);

        // 读回实际发生的情况，而不是复用移动前
        // 值：`move_entity` 会在碰撞时钳制，且爆炸可能已经
        // 在我们上方移动时推了我们一下
        let velo = entity.velocity.load();
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
        } else {
            entity.velocity.store(velo.multiply(0.98, 0.98, 0.98));
        }

        if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            entity.send_pos_rot();
            entity.send_velocity();
        }

        // FIX: 防止引信下溢（与原版一致）
        let fuse = self.fuse.load(Relaxed);

        if fuse <= 1 {
            // TNT 现在爆炸
            self.entity.remove();
            let world = self.entity.world.load_full();
            let pos = self.entity.pos.load();
            let power = self.power;
            if world.level_info.load().game_rules.tnt_explodes {
                world.explode(pos, power, crate::world::ExplosionInteraction::Tnt);
            }
        } else {
            // 安全的递减
            self.fuse.store(fuse - 1, Relaxed);
            entity.update_fluid_state(caller);
        }
    }

    fn init_data_tracker(&self) {
        let pos: f64 = rand::random::<f64>() * TAU;

        self.entity
            .set_velocity(Vector3::new(-pos.sin() * 0.02, 0.2, -pos.cos() * 0.02));

        self.entity.set_synced_data(
            papokin_data::tracked_data::tnt::FUSE_ID,
            VarInt(self.fuse.load(Relaxed) as i32),
        );
        self.entity.set_synced_data(
            papokin_data::tracked_data::tnt::BLOCK_STATE_ID,
            VarInt(i32::from(Block::TNT.default_state.id.as_u16())),
        );
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_gravity(&self) -> f64 {
        0.04
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
