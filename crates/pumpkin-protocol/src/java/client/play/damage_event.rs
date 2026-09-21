use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use pumpkin_data::packet::clientbound::play::DAMAGE_EVENT;
use pumpkin_macros::java_packet;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::version::JavaMinecraftVersion;

/// Notifies the client that an entity has taken damage.
///
/// This packet is used to trigger damage animations (like the red tint on mobs),
/// directional knockback visuals, and sound effects. It provides the client
/// with specific details about the damage source to ensure the visual feedback
/// matches the cause.
#[java_packet(DAMAGE_EVENT)]
pub struct CDamageEvent {
    /// The Entity ID of the entity taking damage.
    pub entity_id: VarInt,
    /// The ID of the damage type (references the `minecraft:damage_type` registry).
    /// Examples: `magic`, `fall`, `on_fire`, or `arrow`.
    pub source_type_id: VarInt,
    /// The Entity ID of the actual cause of the damage (e.g., the player who shot the arrow).
    /// Set to 0 if there is no specific entity cause.
    pub source_cause_id: VarInt,
    /// The Entity ID of the direct damager (e.g., the arrow entity itself).
    /// Set to 0 if this is the same as the cause or if not applicable.
    pub source_direct_id: VarInt,
    /// The coordinates of the damage source. Used by the client to calculate
    /// the direction of the "damage tilt" camera effect.
    pub source_position: Option<Vector3<f64>>,
}

impl CDamageEvent {
    #[must_use]
    pub fn new(
        entity_id: VarInt,
        source_type_id: VarInt,
        source_cause_id: Option<VarInt>,
        source_direct_id: Option<VarInt>,
        source_position: Option<Vector3<f64>>,
    ) -> Self {
        Self {
            entity_id,
            source_type_id,
            source_cause_id: source_cause_id.map_or(VarInt(0), |id| VarInt(id.0 + 1)),
            source_direct_id: source_direct_id.map_or(VarInt(0), |id| VarInt(id.0 + 1)),
            source_position,
        }
    }
}

impl ClientPacket for CDamageEvent {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        // Damage types are a synced registry; translate the dataset id into
        // the id space this client received at configuration time. Vanilla ids
        // go through the static remap tables; ids of plugin-registered custom
        // entries (>= the native vanilla count) are shifted by the difference
        // in vanilla entry counts, or degraded to `generic` for clients that
        // never received the custom entries.
        let source_type_id = pumpkin_data::damage_ext::translate_damage_type_id_for_version(
            u16::try_from(self.source_type_id.0).unwrap_or(0),
            *version,
        );
        write.write_var_int(&self.entity_id)?;
        write.write_var_int(&VarInt(i32::from(source_type_id)))?;
        write.write_var_int(&self.source_cause_id)?;
        write.write_var_int(&self.source_direct_id)?;
        if let Some(pos) = &self.source_position {
            write.write_bool(true)?;
            write.write_f64(pos.x)?;
            write.write_f64(pos.y)?;
            write.write_f64(pos.z)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::damage::DamageType;
    use pumpkin_data::sync_id_remap::remap_damage_type_id_for_version;

    fn serialize(version: JavaMinecraftVersion) -> Vec<u8> {
        let packet = CDamageEvent::new(
            VarInt(1),
            VarInt(i32::from(DamageType::SULFUR_CUBE_HOT.id)),
            None,
            None,
            None,
        );
        let mut buf = Vec::new();
        packet.write_packet_data(&mut buf, &version).unwrap();
        buf
    }

    #[test]
    fn damage_event_remaps_source_type_for_1_21_11() {
        // `sulfur_cube_hot` exists only in the 26.3 dataset; a 1.21.11 client
        // must never see its raw id. entity id (varint 1) comes first, the
        // damage type id second — both single-byte varints here.
        let native = serialize(JavaMinecraftVersion::V_26_3);
        let old = serialize(JavaMinecraftVersion::V_1_21_11);

        assert_eq!(native[1], DamageType::SULFUR_CUBE_HOT.id);
        let expected = remap_damage_type_id_for_version(
            u16::from(DamageType::SULFUR_CUBE_HOT.id),
            JavaMinecraftVersion::V_1_21_11,
        );
        assert_ne!(expected, u16::from(DamageType::SULFUR_CUBE_HOT.id));
        assert_eq!(old[1], u8::try_from(expected).unwrap());
    }

    #[test]
    fn damage_event_translates_custom_ids_per_version() {
        use pumpkin_data::damage_ext::{
            damage_type_vanilla_count_for_version, native_damage_type_count,
        };

        // The first plugin-registered custom damage type sits right after the
        // vanilla entries in the native id space.
        let native_count = native_damage_type_count();
        let packet =
            CDamageEvent::new(VarInt(1), VarInt(i32::from(native_count)), None, None, None);
        let serialize_with = |version: JavaMinecraftVersion| {
            let mut buf = Vec::new();
            packet.write_packet_data(&mut buf, &version).unwrap();
            buf
        };

        // A 1.21.11 client receives the custom entry after *its* vanilla
        // entries, not after the native ones.
        let vanilla_1_21_11 =
            damage_type_vanilla_count_for_version(JavaMinecraftVersion::V_1_21_11).unwrap();
        let old = serialize_with(JavaMinecraftVersion::V_1_21_11);
        // entity id (varint 1) comes first, the damage type id second — both
        // single-byte varints at these values.
        assert_eq!(old[1], u8::try_from(vanilla_1_21_11).unwrap());

        // In the native id space the id is sent unchanged.
        let native = serialize_with(JavaMinecraftVersion::V_26_3);
        assert_eq!(native[1], u8::try_from(native_count).unwrap());

        // A client whose registries are not synced through the configuration
        // state (1.20/1.20.1) never received the custom entry and gets the
        // version's `generic` id instead of an out-of-range one.
        let legacy = serialize_with(JavaMinecraftVersion::V_1_20);
        let expected_generic = remap_damage_type_id_for_version(
            u16::from(DamageType::GENERIC.id),
            JavaMinecraftVersion::V_1_20,
        );
        assert_eq!(legacy[1], u8::try_from(expected_generic).unwrap());
    }
}
