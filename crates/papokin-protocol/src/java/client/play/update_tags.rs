use std::io::Write;

use crate::packet::MultiVersionJavaPacket;
use crate::{ClientPacket, WritingError, ser::NetworkWriteExt, tag_overlay::MergedTags};

use crate::codec::var_int::VarInt;
use papokin_data::{
    packet::clientbound::play::UPDATE_TAGS,
    tag::{RegistryKey, get_registry_key_tags},
};
use papokin_util::version::JavaMinecraftVersion;

pub struct CUpdateTagsPlay<'a> {
    pub tags: &'a [papokin_data::tag::RegistryKey],
    /// 服务器合并后的标签映射（静态表 + 插件覆盖层）。注册表键
    /// 此处存在的条目按原样写入；它们的 id 已经
    /// 版本已解析，因此跳过静态表查找与重映射
    /// 针对它们。
    pub merged: Option<&'a MergedTags>,
}

impl MultiVersionJavaPacket for CUpdateTagsPlay<'_> {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        let id = UPDATE_TAGS.to_id(version);
        if id != -1 {
            return id;
        }
        #[allow(clippy::match_same_arms)]
        match version {
            JavaMinecraftVersion::V_1_20_5 => 0x78,
            JavaMinecraftVersion::V_1_20_3 => 0x74,
            JavaMinecraftVersion::V_1_20_2 => 0x70,
            JavaMinecraftVersion::V_1_20 | JavaMinecraftVersion::V_1_19_4 => 0x6E,
            JavaMinecraftVersion::V_1_19_3 => 0x6A,
            JavaMinecraftVersion::V_1_19_1 => 0x6B,
            JavaMinecraftVersion::V_1_19 => 0x68,
            JavaMinecraftVersion::V_1_18_2 | JavaMinecraftVersion::V_1_18 => 0x67,
            JavaMinecraftVersion::V_1_17_1 | JavaMinecraftVersion::V_1_17 => 0x66,
            JavaMinecraftVersion::V_1_16_4
            | JavaMinecraftVersion::V_1_16_3
            | JavaMinecraftVersion::V_1_16_2
            | JavaMinecraftVersion::V_1_16_1
            | JavaMinecraftVersion::V_1_16
            | JavaMinecraftVersion::V_1_14_4
            | JavaMinecraftVersion::V_1_14_3
            | JavaMinecraftVersion::V_1_14_2
            | JavaMinecraftVersion::V_1_14_1
            | JavaMinecraftVersion::V_1_14 => 0x5B,
            JavaMinecraftVersion::V_1_15_2
            | JavaMinecraftVersion::V_1_15_1
            | JavaMinecraftVersion::V_1_15 => 0x5C,
            JavaMinecraftVersion::V_1_13_2
            | JavaMinecraftVersion::V_1_13_1
            | JavaMinecraftVersion::V_1_13 => 0x55,
            _ => -1,
        }
    }
}

impl<'a> CUpdateTagsPlay<'a> {
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
        p.write_string_bounded(tag_name, u16::MAX as usize)?;
        p.write_list(ids, |p, id| p.write_var_int(&VarInt::from(*id)))?;
    }
    Ok(())
}

impl ClientPacket for CUpdateTagsPlay<'_> {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if version < &JavaMinecraftVersion::V_1_13 {
            return Ok(());
        }

        if version < &JavaMinecraftVersion::V_1_17 {
            // 1.13 至 1.16.5 固定类别格式，不含注册表标识符字符串：
            // 1.13 - 1.13.2：3 个类别（Block、Item、Fluid）
            // 1.14 - 1.16.5：4 个类别（Block、Item、Fluid、EntityType）
            let categories = if version < &JavaMinecraftVersion::V_1_14 {
                &[RegistryKey::Block, RegistryKey::Item, RegistryKey::Fluid][..]
            } else {
                &[
                    RegistryKey::Block,
                    RegistryKey::Item,
                    RegistryKey::Fluid,
                    RegistryKey::EntityType,
                ][..]
            };

            for &key in categories {
                if let Some(entries) = self.merged.and_then(|merged| merged.get(key)) {
                    // 受覆盖层影响的注册表：ID 已解析完成。
                    write_merged_tags(&mut write, entries)?;
                    continue;
                }
                let Some(values) = get_registry_key_tags(*version, key) else {
                    write.write_var_int(&VarInt::from(0))?;
                    continue;
                };
                write.write_var_int(&VarInt(values.len() as i32))?;
                for (tag_name, tag_val) in values.entries() {
                    write.write_string_bounded(tag_name, u16::MAX as usize)?;
                    let remapped_ids: Vec<u16> = tag_val
                        .1
                        .iter()
                        .map(|&id| remap_tag_entry_id(key, id, *version))
                        .collect();
                    write.write_list(&remapped_ids, |p, id| p.write_var_int(&VarInt::from(*id)))?;
                }
            }
            return Ok(());
        }

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
