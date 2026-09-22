use std::io::Write;

use papokin_data::{
    packet::clientbound::play::SOUND_ENTITY, sound::SoundCategory,
    sound_id_remap::remap_sound_id_for_version,
};
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{ClientPacket, IdOr, SoundEvent, VarInt, WritingError, ser::NetworkWriteExt};

/// 播放源自特定实体的音效。
///
/// 与全局声音不同，此声音会跟随实体一起移动
/// 在世界中传播。客户端负责处理声像平移与衰减
/// (音量衰减) 基于玩家与实体之间的距离。
#[java_packet(SOUND_ENTITY)]
pub struct CEntitySoundEffect {
    /// 要播放的声音。可以是硬编码 ID，也可以是自定义 `SoundEvent`
    /// (资源位置)。
    pub sound_event: IdOr<SoundEvent>,
    /// 声音的类别（例如 Master、Music、Weather、Players）。
    /// 客户端用它来应用设置中的音量滑块。
    pub sound_category: VarInt,
    /// 声音所"附着"的实体 ID。
    pub entity_id: VarInt,
    /// 声音的响度（通常为 1.0）。
    pub volume: f32,
    /// 播放速度/音高（0.5 到 2.0）。
    pub pitch: f32,
    /// 用于声音变化的随机种子（例如不同的音高偏移
    /// 针对同一声音）。
    pub seed: i64,
}

impl CEntitySoundEffect {
    #[must_use]
    pub const fn new(
        sound_event: IdOr<SoundEvent>,
        sound_category: SoundCategory,
        entity_id: VarInt,
        volume: f32,
        pitch: f32,
        seed: i64,
    ) -> Self {
        Self {
            sound_event,
            sound_category: VarInt(sound_category as i32),
            entity_id,
            volume,
            pitch,
            seed,
        }
    }
}

impl ClientPacket for CEntitySoundEffect {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if *version >= JavaMinecraftVersion::V_1_19_3 {
            let sound_event = match &self.sound_event {
                IdOr::Id(id) => IdOr::Id(remap_sound_id_for_version(*id, *version)),
                IdOr::Value(value) => IdOr::Value(value.clone()),
            };

            crate::IdOr::<crate::SoundEvent>::write(&sound_event, &mut write, |w, e| {
                w.write_string(&e.sound_name)?;
                w.write_option(&e.range, |w2, r| w2.write_f32(*r))
            })?;
        } else {
            let sound_id = match &self.sound_event {
                IdOr::Id(id) => remap_sound_id_for_version(*id, *version),
                IdOr::Value(_) => 0,
            };
            write.write_var_int(&VarInt(i32::from(sound_id)))?;
        }

        write.write_var_int(&self.sound_category)?;
        write.write_var_int(&self.entity_id)?;
        write.write_f32(self.volume)?;

        if *version >= JavaMinecraftVersion::V_1_10 {
            write.write_f32(self.pitch)?;
        } else {
            let pitch_byte = (self.pitch * 63.0).round().clamp(0.0, 255.0) as u8;
            write.write_u8(pitch_byte)?;
        }

        if *version >= JavaMinecraftVersion::V_1_19 {
            write.write_i64(self.seed)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use papokin_data::sound::SoundCategory;
    use papokin_data::sound_id_remap::remap_sound_id_for_version;
    use papokin_util::version::JavaMinecraftVersion;

    use crate::{ClientPacket, IdOr, SoundEvent, VarInt};

    use super::CEntitySoundEffect;

    fn first_remapped_sound_id(version: JavaMinecraftVersion) -> u16 {
        (0..=u16::MAX)
            .find(|id| remap_sound_id_for_version(*id, version) != *id)
            .expect("音效重映射表应当至少包含一个变更的 id")
    }

    fn first_var_int(bytes: Vec<u8>) -> VarInt {
        VarInt::decode(&mut Cursor::new(bytes)).unwrap()
    }

    #[test]
    fn numeric_sound_id_remaps_for_1_21_11() {
        let sound_id = first_remapped_sound_id(JavaMinecraftVersion::V_1_21_11);
        let packet = CEntitySoundEffect::new(
            IdOr::Id(sound_id),
            SoundCategory::Players,
            VarInt(123),
            1.0,
            1.0,
            42,
        );
        let mut bytes = Vec::new();

        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_1_21_11)
            .unwrap();

        assert_eq!(
            first_var_int(bytes),
            VarInt::from(remap_sound_id_for_version(sound_id, JavaMinecraftVersion::V_1_21_11) + 1)
        );
    }

    #[test]
    fn numeric_sound_id_stays_latest_for_26_2() {
        let sound_id = first_remapped_sound_id(JavaMinecraftVersion::V_1_21_11);
        let packet = CEntitySoundEffect::new(
            IdOr::Id(sound_id),
            SoundCategory::Players,
            VarInt(123),
            1.0,
            1.0,
            42,
        );
        let mut bytes = Vec::new();

        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_26_2)
            .unwrap();

        assert_eq!(first_var_int(bytes), VarInt::from(sound_id + 1));
    }

    #[test]
    fn direct_sound_event_keeps_direct_holder_encoding() {
        let packet = CEntitySoundEffect::new(
            IdOr::Value(SoundEvent {
                sound_name: "minecraft:test.sound".to_string(),
                range: None,
            }),
            SoundCategory::Players,
            VarInt(123),
            1.0,
            1.0,
            42,
        );
        let mut bytes = Vec::new();

        packet
            .write_packet_data(&mut bytes, &JavaMinecraftVersion::V_1_21_11)
            .unwrap();

        assert_eq!(first_var_int(bytes), VarInt::from(0));
    }
}
