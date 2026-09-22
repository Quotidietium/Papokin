use papokin_data::{
    packet::clientbound::play::EXPLODE, sound_id_remap::remap_sound_id_for_version,
};
use papokin_macros::java_packet;
use papokin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

use crate::ser::NetworkWriteExt;
use crate::{ClientPacket, IdOr, SoundEvent, codec::var_int::VarInt};

use super::particle::particle_id_for_version;

/// 通知客户端发生了爆炸。
///
/// 这是一个高层数据包，处理视觉、听觉和物理
/// 单次调用中产生爆炸的全部效果。它会触发爆炸粒子、
/// 在源头播放音效，并对玩家施加击退。
#[java_packet(EXPLODE)]
#[derive(Clone, PartialEq)]
pub struct CExplosion {
    /// 爆炸的中心坐标。
    pub center: Vector3<f64>,
    /// 爆炸的强度/半径。
    /// 数值越大，粒子效果的视觉尺寸越大。
    pub radius: f32,
    /// 受影响/被破坏的方块数量。
    pub block_count: i32,
    /// 施加于接收此数据包的玩家的冲量/击退。
    /// 若为 None，则不应用任何速度变化。
    pub knockback: Option<Vector3<f64>>,
    /// 用于爆炸的粒子 ID（例如 `minecraft:explosion_emitter`）。
    pub particle: VarInt,
    /// 要播放的声音（例如 `minecraft:entity.generic.explode`）。
    pub sound: IdOr<SoundEvent>,
    /// 方块粒子池的大小，用于 1.21.9+ 中的碎屑视觉效果。
    pub block_particles_pool_size: VarInt,
}

impl CExplosion {
    #[must_use]
    pub const fn new(
        center: Vector3<f64>,
        radius: f32,
        block_count: i32,
        knockback: Option<Vector3<f64>>,
        particle: VarInt,
        sound: IdOr<SoundEvent>,
    ) -> Self {
        Self {
            center,
            radius,
            block_count,
            knockback,
            particle,
            sound,
            block_particles_pool_size: VarInt(0),
        }
    }
}

impl ClientPacket for CExplosion {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if *version >= JavaMinecraftVersion::V_1_19_3 {
            write.write_f64_be(self.center.x)?;
            write.write_f64_be(self.center.y)?;
            write.write_f64_be(self.center.z)?;
        } else {
            write.write_f32_be(self.center.x as f32)?;
            write.write_f32_be(self.center.y as f32)?;
            write.write_f32_be(self.center.z as f32)?;
        }

        if *version >= JavaMinecraftVersion::V_1_21_2 {
            if *version >= JavaMinecraftVersion::V_1_21_9 {
                write.write_f32_be(self.radius)?;
                write.write_i32_be(self.block_count)?;
            }

            write.write_option(&self.knockback, |w, k| {
                w.write_f64_be(k.x)?;
                w.write_f64_be(k.y)?;
                w.write_f64_be(k.z)?;
                Ok(())
            })?;

            let particle = particle_id_for_version(self.particle, *version);
            write.write_var_int(&particle)?;

            let sound_event = match &self.sound {
                IdOr::Id(id) => IdOr::Id(remap_sound_id_for_version(*id, *version)),
                IdOr::Value(value) => IdOr::Value(value.clone()),
            };
            crate::IdOr::<crate::SoundEvent>::write(&sound_event, &mut write, |w, e| {
                w.write_string(&e.sound_name)?;
                w.write_option(&e.range, |w2, r| w2.write_f32_be(*r))
            })?;

            if *version >= JavaMinecraftVersion::V_1_21_9 {
                write.write_var_int(&self.block_particles_pool_size)?;
            }

            // 是否播放爆炸音效，26.3 新增
            if *version >= JavaMinecraftVersion::V_26_3 {
                write.write_bool(true)?;
            }
        } else {
            write.write_f32_be(self.radius)?;

            if *version >= JavaMinecraftVersion::V_1_17 {
                write.write_var_int(&VarInt(0))?;
            } else {
                write.write_i32_be(0)?;
            }

            if let Some(knockback) = self.knockback {
                write.write_f32_be(knockback.x as f32)?;
                write.write_f32_be(knockback.y as f32)?;
                write.write_f32_be(knockback.z as f32)?;
            } else {
                write.write_f32_be(0.0)?;
                write.write_f32_be(0.0)?;
                write.write_f32_be(0.0)?;
            }

            if *version >= JavaMinecraftVersion::V_1_20_3 {
                // 方块交互：1 = DESTROY_BLOCKS
                write.write_var_int(&VarInt(1))?;

                let small_particle = particle_id_for_version(
                    VarInt(papokin_data::particle::Particle::Explosion as i32),
                    *version,
                );
                write.write_var_int(&small_particle)?;

                let particle = particle_id_for_version(self.particle, *version);
                write.write_var_int(&particle)?;

                if *version >= JavaMinecraftVersion::V_1_20_5 {
                    let sound_event = match &self.sound {
                        IdOr::Id(id) => IdOr::Id(remap_sound_id_for_version(*id, *version)),
                        IdOr::Value(value) => IdOr::Value(value.clone()),
                    };
                    crate::IdOr::<crate::SoundEvent>::write(&sound_event, &mut write, |w, e| {
                        w.write_string(&e.sound_name)?;
                        w.write_option(&e.range, |w2, r| w2.write_f32_be(*r))
                    })?;
                } else {
                    let (sound_name, range) = match &self.sound {
                        IdOr::Id(id) => {
                            let remapped = remap_sound_id_for_version(*id, *version);
                            let name = papokin_data::sound::Sound::NAMES
                                .get(remapped as usize)
                                .copied()
                                .unwrap_or("minecraft:entity.generic.explode");
                            (name, None)
                        }
                        IdOr::Value(event) => (event.sound_name.as_str(), event.range),
                    };
                    write.write_string(sound_name)?;
                    write.write_option(&range, |w, r| w.write_f32_be(*r))?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek, SeekFrom};

    use papokin_data::particle::Particle;
    use papokin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

    use crate::{ClientPacket, IdOr, VarInt};

    use super::CExplosion;

    fn encoded_particle_id(version: JavaMinecraftVersion) -> VarInt {
        let packet = CExplosion::new(
            Vector3::new(0.0, 0.0, 0.0),
            4.0,
            0,
            None,
            VarInt(Particle::ExplosionEmitter as i32),
            IdOr::Id(0),
        );
        let mut bytes = Vec::new();
        packet.write_packet_data(&mut bytes, &version).unwrap();

        let mut cursor = Cursor::new(bytes);
        cursor.seek(SeekFrom::Start(33)).unwrap();
        VarInt::decode(&mut cursor).unwrap()
    }

    #[test]
    fn explosion_particle_id_remaps_for_1_21_11() {
        assert_eq!(
            encoded_particle_id(JavaMinecraftVersion::V_1_21_11),
            VarInt(22)
        );
    }

    #[test]
    fn explosion_particle_id_stays_latest_for_26_2() {
        assert_eq!(
            encoded_particle_id(JavaMinecraftVersion::V_26_2),
            VarInt(29)
        );
    }
}
