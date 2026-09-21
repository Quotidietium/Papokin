use std::io::Write;

use crate::{ClientPacket, WritingError, ser::NetworkWriteExt, tag_overlay::MergedTags};

use crate::codec::var_int::VarInt;
use pumpkin_data::{
    packet::clientbound::config::UPDATE_TAGS,
    tag::{RegistryKey, get_registry_key_tags},
};
use pumpkin_macros::java_packet;
use pumpkin_util::version::JavaMinecraftVersion;

#[java_packet(UPDATE_TAGS)]
pub struct CUpdateTags<'a> {
    pub tags: &'a [pumpkin_data::tag::RegistryKey],
    /// Server-merged tag maps (static table + plugin overlay). Registry keys
    /// present here are written verbatim; their ids are already
    /// version-resolved, so the static-table lookup and remapping are skipped
    /// for them.
    pub merged: Option<&'a MergedTags>,
}

impl<'a> CUpdateTags<'a> {
    #[must_use]
    pub const fn new(tags: &'a [RegistryKey]) -> Self {
        Self { tags, merged: None }
    }

    #[must_use]
    pub const fn with_merged(tags: &'a [RegistryKey], merged: Option<&'a MergedTags>) -> Self {
        Self { tags, merged }
    }
}

fn remap_tag_entry_id(key: RegistryKey, id: u16, version: JavaMinecraftVersion) -> u16 {
    match key {
        RegistryKey::Item => pumpkin_data::item_id_remap::remap_item_id_for_version(id, version),
        RegistryKey::EntityType => {
            pumpkin_data::entity_id_remap::remap_entity_id_for_version(id, version)
        }
        _ => id,
    }
}

/// Writes one registry key's tag map as `(tag name, id list)` entries whose
/// ids are already resolved for the client version.
fn write_merged_tags(
    p: &mut impl Write,
    entries: &[(String, Vec<u16>)],
) -> Result<(), WritingError> {
    p.write_var_int(&VarInt::from(i32::try_from(entries.len()).map_err(
        |_| WritingError::Message(format!("{} isn't representable as a VarInt", entries.len())),
    )?))?;
    for (tag_name, ids) in entries {
        // This is technically a `ResourceLocation` but same thing
        p.write_string_bounded(tag_name, u16::MAX as usize)?;
        p.write_list(ids, |p, id| p.write_var_int(&VarInt::from(*id)))?;
    }
    Ok(())
}

impl ClientPacket for CUpdateTags<'_> {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let valid_keys: Vec<_> = self
            .tags
            .iter()
            .copied()
            .filter(|key| key.is_valid_for_version(*version))
            .collect();

        write.write_list(&valid_keys, |p, &registry_key| {
            p.write_string(&format!("minecraft:{}", registry_key.identifier_string()))?;

            if let Some(entries) = self.merged.and_then(|merged| merged.get(registry_key)) {
                // Overlay-touched registry: the server already merged the
                // static table with the plugin overlay and resolved every id.
                return write_merged_tags(p, entries);
            }

            let Some(values) = get_registry_key_tags(*version, registry_key) else {
                // no tags defined for that registry key in this version
                // write an empty list and continue
                p.write_var_int(&VarInt::from(0))?;
                return Ok(());
            };
            p.write_var_int(&values.len().try_into().map_err(|_| {
                WritingError::Message(format!("{} isn't representable as a VarInt", values.len()))
            })?)?;

            for (key, values) in values.entries() {
                // This is technically a `ResourceLocation` but same thing
                p.write_string_bounded(key, u16::MAX as usize)?;
                let remapped_ids: Vec<u16> = values
                    .1
                    .iter()
                    .map(|&id| remap_tag_entry_id(registry_key, id, *version))
                    .collect();
                p.write_list(&remapped_ids, |p, id| p.write_var_int(&VarInt::from(*id)))?;
            }

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn serialize(packet: &CUpdateTags, version: JavaMinecraftVersion) -> Vec<u8> {
        let mut buf = Vec::new();
        packet.write_packet_data(&mut buf, &version).unwrap();
        buf
    }

    #[test]
    fn static_path_is_byte_identical_without_merged() {
        let tags = [RegistryKey::Item];
        let plain = serialize(&CUpdateTags::new(&tags), JavaMinecraftVersion::V_1_21);
        let with_none = serialize(
            &CUpdateTags::with_merged(&tags, None),
            JavaMinecraftVersion::V_1_21,
        );
        assert_eq!(plain, with_none);
    }

    #[test]
    fn merged_entries_replace_the_static_table_verbatim() {
        let version = JavaMinecraftVersion::V_1_21;
        let tags = [RegistryKey::DamageType];
        let mut maps = HashMap::new();
        // Id 0x7FFF would be remapped/clamped on the static path for remapped
        // registries; merged ids must be written exactly as given.
        maps.insert(
            RegistryKey::DamageType,
            vec![("myplugin:everything".to_string(), vec![1, 0x7FFF])],
        );
        let merged = MergedTags { maps };
        let bytes = serialize(&CUpdateTags::with_merged(&tags, Some(&merged)), version);

        let plain = serialize(&CUpdateTags::new(&tags), version);
        assert_ne!(bytes, plain);
        // Tag name appears verbatim in the output.
        let needle = b"myplugin:everything";
        assert!(bytes.windows(needle.len()).any(|window| window == needle));
        // The static damage_type tag names must be gone.
        assert!(!bytes.windows(7).any(|window| window == b"is_fire"));
    }

    #[test]
    fn untagged_keys_fall_back_to_the_static_path() {
        let version = JavaMinecraftVersion::V_1_21;
        let tags = [RegistryKey::Item, RegistryKey::DamageType];
        let mut maps = HashMap::new();
        maps.insert(
            RegistryKey::DamageType,
            vec![("myplugin:everything".to_string(), vec![0])],
        );
        let merged = MergedTags { maps };
        let bytes = serialize(&CUpdateTags::with_merged(&tags, Some(&merged)), version);
        // Item fell back to the static path: a static item tag name survives.
        let needle = b"minecraft:anvil";
        assert!(bytes.windows(needle.len()).any(|window| window == needle));
    }
}
