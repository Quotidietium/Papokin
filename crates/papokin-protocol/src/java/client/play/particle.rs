use std::io::Write;

use papokin_data::{
    packet::clientbound::play::LEVEL_PARTICLES, particle_id_remap::remap_particle_id_for_version,
};
use papokin_macros::java_packet;
use papokin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkReadSliceExt, NetworkWriteExt, ReadingError, WritingError},
};

/// 在特定位置生成一簇粒子。
///
/// 这是协议中最通用的视觉数据包。它可以实现
/// 精确控制粒子密度、扩散范围和速度。它还可以
/// 为复杂粒子携带额外数据，例如红石粉（颜色）或
/// 方块/物品破坏（纹理）。
#[java_packet(LEVEL_PARTICLES)]
#[derive(Clone, Debug, PartialEq)]
pub struct CParticle<'a> {
    /// 如果为 true，即使客户端的“粒子”
    /// 设置项被设为 "Minimal" 时。
    pub force_spawn: bool,
    /// 如果为 true，粒子可见距离会显著
    /// 扩大（从 256 块增加到 65536 块）。常用于大型活动。
    pub important: bool,
    /// 粒子群的绝对中心位置。
    pub position: Vector3<f64>,
    /// 粒子距中心可生成的最大距离。
    pub offset: Vector3<f32>,
    /// 粒子的速度，即“散布”速度。
    pub max_speed: f32,
    /// 此粒子簇中要生成的粒子总数。
    pub particle_count: i32,
    /// 粒子类型的 ID（例如 `minecraft:flame`）。
    pub particle_id: VarInt,
    /// 特定粒子所需的额外数据（例如某些粒子所用的方块状态
    /// `block` 粒子，或 `dust` 所需的 RGB 值）。
    pub data: &'a [u8],
}

impl<'a> CParticle<'a> {
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        force_spawn: bool,
        important: bool,
        position: Vector3<f64>,
        offset: Vector3<f32>,
        max_speed: f32,
        particle_count: i32,
        particle_id: VarInt,
        data: &'a [u8],
    ) -> Self {
        Self {
            force_spawn,
            important,
            position,
            offset,
            max_speed,
            particle_count,
            particle_id,
            data,
        }
    }
}

#[must_use]
pub const fn particle_name_for_v1_7(particle: papokin_data::particle::Particle) -> &'static str {
    use papokin_data::particle::Particle::{
        AngryVillager, Block, BlockCrumble, BlockMarker, Bubble, BubbleColumnUp, BubblePop,
        CampfireSignalSmoke, Cloud, Composter, Crit, DamageIndicator, DragonBreath,
        DrippingDripstoneLava, DrippingDripstoneWater, DrippingLava, DrippingWater, Dust,
        DustColorTransition, DustPillar, DustPlume, Effect, Enchant, EnchantedHit, EntityEffect,
        Explosion, ExplosionEmitter, FallingDust, Firework, Fishing, Flame, HappyVillager, Heart,
        InstantEffect, Item, ItemSlime, ItemSnowball, LargeSmoke, Lava, Mycelium, Note, Poof,
        Portal, Rain, ReversePortal, SmallFlame, Snowflake, SoulFireFlame, Splash, SweepAttack,
        TotemOfUndying, Underwater, Witch,
    };
    match particle {
        ExplosionEmitter => "hugeexplosion",
        Explosion => "largeexplode",
        Poof => "explode",
        Firework => "fireworksSpark",
        Bubble | BubblePop | BubbleColumnUp => "bubble",
        Splash => "splash",
        Fishing => "wake",
        Underwater => "suspended",
        Crit | DamageIndicator | SweepAttack => "crit",
        EnchantedHit => "magicCrit",
        LargeSmoke | CampfireSignalSmoke => "largesmoke",
        InstantEffect => "spell",
        EntityEffect => "mobSpell",
        Effect => "mobSpellAmbient",
        Witch | TotemOfUndying | DragonBreath => "witchMagic",
        DrippingWater | DrippingDripstoneWater => "dripWater",
        DrippingLava | DrippingDripstoneLava => "dripLava",
        AngryVillager => "angryVillager",
        HappyVillager | Composter => "happyVillager",
        Mycelium => "townaura",
        Note => "note",
        Portal | ReversePortal => "portal",
        Enchant => "enchantmenttable",
        Flame | SmallFlame | SoulFireFlame => "flame",
        Lava => "lava",
        Cloud => "cloud",
        Dust | DustColorTransition | DustPillar | DustPlume => "reddust",
        ItemSnowball | Snowflake => "snowballpoof",
        ItemSlime => "slime",
        Heart => "heart",
        BlockMarker => "barrier",
        Rain => "droplet",
        Item => "iconcrack_",
        Block | BlockCrumble => "blockcrack_",
        FallingDust => "blockdust_",
        _ => "smoke",
    }
}

#[must_use]
pub fn particle_id_from_1_7_name(name: &str) -> i32 {
    match name {
        "explode" => papokin_data::particle::Particle::Poof as i32,
        "largeexplode" => papokin_data::particle::Particle::Explosion as i32,
        "hugeexplosion" => papokin_data::particle::Particle::ExplosionEmitter as i32,
        "fireworksSpark" => papokin_data::particle::Particle::Firework as i32,
        "bubble" => papokin_data::particle::Particle::Bubble as i32,
        "splash" => papokin_data::particle::Particle::Splash as i32,
        "wake" => papokin_data::particle::Particle::Fishing as i32,
        "suspended" | "depthsuspend" => papokin_data::particle::Particle::Underwater as i32,
        "crit" => papokin_data::particle::Particle::Crit as i32,
        "magicCrit" => papokin_data::particle::Particle::EnchantedHit as i32,
        "smoke" => papokin_data::particle::Particle::Smoke as i32,
        "largesmoke" => papokin_data::particle::Particle::LargeSmoke as i32,
        "spell" | "instantSpell" => papokin_data::particle::Particle::InstantEffect as i32,
        "mobSpell" => papokin_data::particle::Particle::EntityEffect as i32,
        "mobSpellAmbient" => papokin_data::particle::Particle::Effect as i32,
        "witchMagic" => papokin_data::particle::Particle::Witch as i32,
        "dripWater" => papokin_data::particle::Particle::DrippingWater as i32,
        "dripLava" => papokin_data::particle::Particle::DrippingLava as i32,
        "angryVillager" => papokin_data::particle::Particle::AngryVillager as i32,
        "happyVillager" => papokin_data::particle::Particle::HappyVillager as i32,
        "townaura" => papokin_data::particle::Particle::Mycelium as i32,
        "note" => papokin_data::particle::Particle::Note as i32,
        "portal" => papokin_data::particle::Particle::Portal as i32,
        "enchantmenttable" => papokin_data::particle::Particle::Enchant as i32,
        "flame" => papokin_data::particle::Particle::Flame as i32,
        "lava" => papokin_data::particle::Particle::Lava as i32,
        "cloud" => papokin_data::particle::Particle::Cloud as i32,
        "reddust" => papokin_data::particle::Particle::Dust as i32,
        "snowballpoof" | "snowshovel" => papokin_data::particle::Particle::ItemSnowball as i32,
        "slime" => papokin_data::particle::Particle::ItemSlime as i32,
        "heart" => papokin_data::particle::Particle::Heart as i32,
        "barrier" => papokin_data::particle::Particle::BlockMarker as i32,
        "droplet" => papokin_data::particle::Particle::Rain as i32,
        _ => papokin_data::particle::Particle::from_name(name).map_or(0, |p| p as i32),
    }
}

pub(super) fn particle_id_for_version(
    particle_id: VarInt,
    version: JavaMinecraftVersion,
) -> VarInt {
    u16::try_from(particle_id.0).map_or(particle_id, |particle_id| {
        VarInt(i32::from(remap_particle_id_for_version(
            particle_id,
            version,
        )))
    })
}

impl ClientPacket for CParticle<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        if *version <= JavaMinecraftVersion::V_1_7_6 {
            let name = papokin_data::particle::Particle::from_id(self.particle_id.0 as u16)
                .map_or("smoke", particle_name_for_v1_7);
            write.write_string_bounded(name, 64)?;
        } else if *version < JavaMinecraftVersion::V_1_20_5 {
            let remapped_id =
                remap_particle_id_for_version(self.particle_id.0 as u16, *version) as i32;
            if *version >= JavaMinecraftVersion::V_1_19 {
                write.write_var_int(&VarInt(remapped_id))?;
            } else {
                write.write_i32_be(remapped_id)?;
            }
        } else if *version >= JavaMinecraftVersion::V_26_3 {
            // 在 26.3 中，粒子被移回了数据包的起始位置
            let remapped_id =
                remap_particle_id_for_version(self.particle_id.0 as u16, *version) as i32;
            write.write_var_int(&VarInt(remapped_id))?;
            write.write_slice(self.data)?;
        }

        if *version >= JavaMinecraftVersion::V_1_8 {
            write.write_bool(self.important)?;
        }
        if *version >= JavaMinecraftVersion::V_1_21_4 {
            write.write_bool(self.force_spawn)?;
        }

        if *version >= JavaMinecraftVersion::V_1_15 {
            write.write_f64_be(self.position.x)?;
            write.write_f64_be(self.position.y)?;
            write.write_f64_be(self.position.z)?;
        } else {
            write.write_f32_be(self.position.x as f32)?;
            write.write_f32_be(self.position.y as f32)?;
            write.write_f32_be(self.position.z as f32)?;
        }

        write.write_f32_be(self.offset.x)?;
        write.write_f32_be(self.offset.y)?;
        write.write_f32_be(self.offset.z)?;

        write.write_f32_be(self.max_speed)?;
        if *version >= JavaMinecraftVersion::V_26_3 {
            // 自 26.3 起，速度按每个轴设置，计数是一个 var int，其后是
            // 随机化类型，0 为默认值。
            write.write_f32_be(self.max_speed)?;
            write.write_f32_be(self.max_speed)?;
            write.write_var_int(&VarInt(self.particle_count))?;
            write.write_var_int(&VarInt(0))?;
            return Ok(());
        }
        write.write_i32_be(self.particle_count)?;

        if *version >= JavaMinecraftVersion::V_1_20_5 {
            let remapped_id =
                remap_particle_id_for_version(self.particle_id.0 as u16, *version) as i32;
            write.write_var_int(&VarInt(remapped_id))?;
        }
        write.write_slice(self.data)?;

        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CParticle<'a> {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let (particle_id, important, force_spawn) = if *version <= JavaMinecraftVersion::V_1_7_6 {
            let name = bytebuf.get_str_bounded_borrowed(64)?;
            let id = particle_id_from_1_7_name(name);
            (VarInt(id), false, false)
        } else if *version < JavaMinecraftVersion::V_1_20_5 {
            let id = if *version >= JavaMinecraftVersion::V_1_19 {
                bytebuf.get_var_int()?
            } else {
                VarInt(bytebuf.get_i32_be()?)
            };
            let important = bytebuf.get_bool()?;
            (id, important, false)
        } else {
            let important = bytebuf.get_bool()?;
            let force_spawn = if *version >= JavaMinecraftVersion::V_1_21_4 {
                bytebuf.get_bool()?
            } else {
                false
            };
            (VarInt(0), important, force_spawn)
        };

        let position = if *version >= JavaMinecraftVersion::V_1_15 {
            Vector3::new(
                bytebuf.get_f64_be()?,
                bytebuf.get_f64_be()?,
                bytebuf.get_f64_be()?,
            )
        } else {
            Vector3::new(
                f64::from(bytebuf.get_f32_be()?),
                f64::from(bytebuf.get_f32_be()?),
                f64::from(bytebuf.get_f32_be()?),
            )
        };

        let offset = Vector3::new(
            bytebuf.get_f32_be()?,
            bytebuf.get_f32_be()?,
            bytebuf.get_f32_be()?,
        );
        let max_speed = bytebuf.get_f32_be()?;
        let particle_count = bytebuf.get_i32_be()?;

        let (particle_id, data) = if *version >= JavaMinecraftVersion::V_1_20_5 {
            let id = bytebuf.get_var_int()?;
            let remaining = bytebuf.read_remaining_slice_borrowed(usize::MAX)?;
            (id, remaining)
        } else {
            let remaining = bytebuf.read_remaining_slice_borrowed(usize::MAX)?;
            (particle_id, remaining)
        };

        Ok(Self {
            force_spawn,
            important,
            position,
            offset,
            max_speed,
            particle_count,
            particle_id,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek, SeekFrom};

    use papokin_data::particle::Particle;
    use papokin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

    use crate::{ClientPacket, VarInt};

    use super::CParticle;

    fn encoded_particle_id(version: JavaMinecraftVersion) -> VarInt {
        let packet = CParticle::new(
            false,
            false,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            0.0,
            1,
            VarInt(Particle::ExplosionEmitter as i32),
            &[],
        );
        let mut bytes = Vec::new();
        packet.write_packet_data(&mut bytes, &version).unwrap();

        let mut cursor = Cursor::new(bytes);
        cursor.seek(SeekFrom::Start(46)).unwrap();
        VarInt::decode(&mut cursor).unwrap()
    }

    #[test]
    fn particle_id_remaps_for_1_21_11() {
        assert_eq!(
            encoded_particle_id(JavaMinecraftVersion::V_1_21_11),
            VarInt(22)
        );
    }

    #[test]
    fn particle_id_stays_latest_for_26_2() {
        assert_eq!(
            encoded_particle_id(JavaMinecraftVersion::V_26_2),
            VarInt(29)
        );
    }

    #[test]
    fn particle_encoding_legacy_1_7_string() {
        use crate::ser::NetworkReadSliceExt;

        let packet = CParticle::new(
            false,
            false,
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(0.1, 0.2, 0.3),
            0.5,
            10,
            VarInt(Particle::ExplosionEmitter as i32),
            &[],
        );
        let mut bytes = Vec::new();
        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_1_7_6)
            .unwrap();

        let mut slice = bytes.as_slice();
        let name = slice.get_str_borrowed().unwrap();
        assert_eq!(name, "hugeexplosion");
    }

    #[test]
    fn particle_encoding_1_8_int_id() {
        use crate::ser::NetworkReadExt;

        let packet = CParticle::new(
            false,
            false,
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(0.1, 0.2, 0.3),
            0.5,
            10,
            VarInt(Particle::ExplosionEmitter as i32),
            &[],
        );
        let mut bytes = Vec::new();
        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_1_8)
            .unwrap();

        let mut slice = bytes.as_slice();
        let id = slice.get_i32_be().unwrap();
        assert_eq!(id, 18);
    }

    #[test]
    fn particle_encoding_1_19_varint_id() {
        let packet = CParticle::new(
            false,
            false,
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(0.1, 0.2, 0.3),
            0.5,
            10,
            VarInt(Particle::ExplosionEmitter as i32),
            &[],
        );
        let mut bytes = Vec::new();
        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_1_19)
            .unwrap();

        let mut cursor = Cursor::new(bytes);
        let id = VarInt::decode(&mut cursor).unwrap();
        assert_eq!(id, VarInt(22));
    }
}
