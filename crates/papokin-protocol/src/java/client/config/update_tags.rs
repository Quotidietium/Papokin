use std::io::Write;

use crate::{ClientPacket, WritingError, ser::NetworkWriteExt, tag_overlay::MergedTags};

use crate::codec::var_int::VarInt;
use papokin_data::{
    packet::clientbound::config::UPDATE_TAGS,
    tag::{RegistryKey, get_registry_key_tags},
};
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(UPDATE_TAGS)]
pub struct CUpdateTags<'a> {
    pub tags: &'a [papokin_data::tag::RegistryKey],
    /// 服务器合并后的标签映射（静态表 + 插件覆盖层）。注册表键
    /// 此处存在的条目按原样写入；它们的 id 已经
    /// 版本已解析，因此跳过静态表查找与重映射
    /// 针对它们。
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
        RegistryKey::Item => papokin_data::item_id_remap::remap_item_id_for_version(id, version),
        RegistryKey::EntityType => {
            papokin_data::entity_id_remap::remap_entity_id_for_version(id, version)
        }
        _ => id,
    }
}

/// 将某个注册表键的标签映射写成 `(tag name, id list)` 条目，这些条目的
/// id 已针对客户端版本解析完成。
fn write_merged_tags(
    p: &mut impl Write,
    entries: &[(String, Vec<u16>)],
) -> Result<(), WritingError> {
    p.write_var_int(&VarInt::from(i32::try_from(entries.len()).map_err(
        |_| WritingError::Message(format!("{} isn't representable as a VarInt", entries.len())),
    )?))?;
    for (tag_name, ids) in entries {
        // 严格来说这是一个 `ResourceLocation`，但反正是一回事
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
                // 受覆盖层影响的注册表：服务器已经合并了
                // 静态表与插件覆盖层，并解析了每一个 id。
                return write_merged_tags(p, entries);
            }

            let Some(values) = get_registry_key_tags(*version, registry_key) else {
                // 该版本中此注册表键未定义任何标签
                // 写入空列表并继续
                p.write_var_int(&VarInt::from(0))?;
                return Ok(());
            };
            p.write_var_int(&values.len().try_into().map_err(|_| {
                WritingError::Message(format!("{} isn't representable as a VarInt", values.len()))
            })?)?;

            for (key, values) in values.entries() {
                // 严格来说这是一个 `ResourceLocation`，但反正是一回事
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
        // Id 0x7FFF 会在重映射的静态路径上被重映射/钳制
        // 注册表；合并后的 id 必须按给定值原样写入。
        maps.insert(
            RegistryKey::DamageType,
            vec![("myplugin:everything".to_string(), vec![1, 0x7FFF])],
        );
        let merged = MergedTags { maps };
        let bytes = serialize(&CUpdateTags::with_merged(&tags, Some(&merged)), version);

        let plain = serialize(&CUpdateTags::new(&tags), version);
        assert_ne!(bytes, plain);
        // 标签名称会原样出现在输出中。
        let needle = b"myplugin:everything";
        assert!(bytes.windows(needle.len()).any(|window| window == needle));
        // 静态的 damage_type 标签名必须移除。
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
        // 物品回退到静态路径：静态物品标签名得以保留。
        let needle = b"minecraft:anvil";
        assert!(bytes.windows(needle.len()).any(|window| window == needle));
    }
}
