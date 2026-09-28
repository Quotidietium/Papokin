use super::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use core::f32;
use papokin_data::Block;
use papokin_nbt::compound::NbtCompound;
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
    /// 爆炸威力，按 f32 位模式原子存取（`read_custom_nbt` 只有
    /// `&self`，重载需要内部可变性）。
    power: AtomicU32,
    fuse: AtomicU32,
}

impl TNTEntity {
    pub const fn new(entity: Entity, power: f32, fuse: u32) -> Self {
        Self {
            entity,
            power: AtomicU32::new(power.to_bits()),
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
            let power = f32::from_bits(self.power.load(Relaxed));
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

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        // 引信饱和而非截断：引信只能来自本实现的有限递减，但防
        // 御性饱和成本为零。
        nbt.put_short("Fuse", self.fuse.load(Relaxed).min(i16::MAX as u32) as i16);
        // 原版威力恒 4 不落盘；本实现允许按实体定制威力，不写回
        // 会在重载后静默回落到默认威力（爆炸 API/发射器等场景）。
        nbt.put_float("PumpkinPower", f32::from_bits(self.power.load(Relaxed)));
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        // 负值（损坏存档）按默认引信处理，不得符号扩展
        self.fuse
            .store(nbt.get_short("Fuse").unwrap_or(80).max(0) as u32, Relaxed);
        if let Some(power) = nbt.get_float("PumpkinPower")
            && power.is_finite()
            && power >= 0.0
        {
            self.power.store(power.to_bits(), Relaxed);
        }
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
