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
        RegistryKey::Block => papokin_data::block_id_remap::remap_block_id_for_version(id, version),
        RegistryKey::EntityType => {
            papokin_data::entity_id_remap::remap_entity_id_for_version(id, version)
        }
        // 流体注册序 1.13–26.x 与数据集同序，id 原样下发（勘误见
        // papokin-data::tag_sync 模块文档：此前的 2↔3 换序会把水装进
        // lava 标签，令客户端浸水时渲染岩浆红屏）。
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
            .filter(|key| {
                key.is_valid_for_version(*version)
                    && papokin_data::tag_sync::tag_registry_sendable_for_version(*key, *version)
            })
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
                    .filter_map(|&id| {
                        let mapped = remap_tag_entry_id(registry_key, id, *version);
                        // 目标版本不存在的条目映射为 0（=air/占位），原版
                        // 客户端的标签里不会出现它们；保留恒等的 0 本体。
                        (id == 0 || mapped != 0).then_some(mapped)
                    })
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

    // ── 方块标签 id 的跨版本重映射 ──────────────────────────────

    fn read_varint(bytes: &[u8], pos: &mut usize) -> i32 {
        let mut value: i32 = 0;
        for i in 0..5 {
            let b = bytes[*pos];
            *pos += 1;
            value |= i32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return value;
            }
        }
        panic!("VarInt 超过 5 字节");
    }

    fn read_string(bytes: &[u8], pos: &mut usize) -> String {
        let len = usize::try_from(read_varint(bytes, pos)).unwrap();
        let s = String::from_utf8(bytes[*pos..*pos + len].to_vec()).unwrap();
        *pos += len;
        s
    }

    /// 线格式解包：标签名 → id 列表（首个注册表 minecraft:block）。
    fn parse_block_tags(bytes: &[u8]) -> HashMap<String, Vec<u16>> {
        let mut pos = 0;
        let registry_count = read_varint(bytes, &mut pos);
        let mut result = HashMap::new();
        for _ in 0..registry_count {
            let _registry_name = read_string(bytes, &mut pos);
            let tag_count = read_varint(bytes, &mut pos);
            for _ in 0..tag_count {
                let name = read_string(bytes, &mut pos);
                let id_count = read_varint(bytes, &mut pos);
                let ids: Vec<u16> = (0..id_count)
                    .map(|_| u16::try_from(read_varint(bytes, &mut pos)).unwrap())
                    .collect();
                result.insert(name, ids);
            }
        }
        result
    }

    #[test]
    fn block_tag_ids_are_rewritten_to_client_version_ids() {
        // 1.21.11 客户端冻结注册表：oak_log=49（数据集 26.3 中为 51）、
        // stone=1。序列化输出的方块标签 id 必须是客户端版本的 id，
        // 否则客户端会把 mineable/* 与 incorrect_for_* 标签整体错读
        // （表现为工具对应错乱，如“橡木显示木镐可挖”）。
        let version = JavaMinecraftVersion::V_1_21_11;
        let tags = [RegistryKey::Block];
        let bytes = serialize(&CUpdateTags::new(&tags), version);
        let block_tags = parse_block_tags(&bytes);

        let axe = &block_tags["minecraft:mineable/axe"];
        assert!(axe.contains(&49), "mineable/axe 应含 oak_log(49)：{axe:?}");
        // 位移证据：数据集 oak_door(265) 在 1.21.11 为 219
        assert!(
            axe.contains(&219),
            "mineable/axe 应含 oak_door(219)：{axe:?}"
        );

        let pickaxe = &block_tags["minecraft:mineable/pickaxe"];
        assert!(pickaxe.contains(&1), "mineable/pickaxe 应含 stone(1)");
        assert!(
            !pickaxe.contains(&49),
            "mineable/pickaxe 不得含 oak_log(49)：{pickaxe:?}"
        );
        // 26.x 独有方块（如 sulfur_bricks）映射为 0，不得泄漏进标签
        assert!(
            !pickaxe.contains(&0) && !axe.contains(&0),
            "标签不得含 0（目标版本缺席条目应被过滤）"
        );
    }

    /// 线格式解包：注册表名 →（标签名 → id 列表）。
    fn parse_all_registries(bytes: &[u8]) -> HashMap<String, HashMap<String, Vec<u16>>> {
        let mut pos = 0;
        let registry_count = read_varint(bytes, &mut pos);
        let mut result = HashMap::new();
        for _ in 0..registry_count {
            let registry_name = read_string(bytes, &mut pos);
            let tag_count = read_varint(bytes, &mut pos);
            let mut tags = HashMap::new();
            for _ in 0..tag_count {
                let name = read_string(bytes, &mut pos);
                let id_count = read_varint(bytes, &mut pos);
                let ids: Vec<u16> = (0..id_count)
                    .map(|_| u16::try_from(read_varint(bytes, &mut pos)).unwrap())
                    .collect();
                tags.insert(name, ids);
            }
            result.insert(registry_name, tags);
        }
        result
    }

    /// `game_event` 标签对 1.21.11 客户端整体省略（id 跨版本漂移且无
    /// 逐版本映射数据，省略时客户端保留内建原版标签）；fluid 注册序
    /// 1.13–26.x 与数据集同序，标签 id 必须原样下发——客户端的眼睛
    /// 入液判定走 FluidTags，lava 标签一旦含水、water 标签不含水，
    /// 浸水就会渲染岩浆红屏且没有水下效果（2026-10-01 勘误回归锚点）。
    #[test]
    fn game_event_omitted_and_fluid_ids_unchanged_for_old_clients() {
        let keys = [
            RegistryKey::Block,
            RegistryKey::Item,
            RegistryKey::Fluid,
            RegistryKey::EntityType,
            RegistryKey::GameEvent,
        ];

        let old = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_21_11,
        ));
        assert!(
            !old.contains_key("minecraft:game_event"),
            "1.21.11 客户端不应收到 game_event 标签：{:?}",
            old.keys().collect::<Vec<_>>()
        );
        // 原版流体注册序：empty=0, flowing_water=1, water=2,
        // flowing_lava=3, lava=4（与数据集一致，无任何换序）。
        let water = &old["minecraft:fluid"]["minecraft:water"];
        assert!(water.contains(&2), "fluid:water 应含 water(2)：{water:?}");
        assert!(
            !water.contains(&3),
            "fluid:water 不得含 flowing_lava(3)：{water:?}"
        );
        let lava = &old["minecraft:fluid"]["minecraft:lava"];
        assert!(
            lava.contains(&3) && lava.contains(&4),
            "fluid:lava 应含 flowing_lava(3) 与 lava(4)：{lava:?}"
        );
        assert!(
            !lava.contains(&2),
            "fluid:lava 不得含 water(2)——含水会让客户端在水下渲染岩浆红屏：{lava:?}"
        );

        let native = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_26_3,
        ));
        assert!(
            native.contains_key("minecraft:game_event"),
            "26.3 原生客户端应收到 game_event 标签"
        );
        let native_water = &native["minecraft:fluid"]["minecraft:water"];
        assert!(
            native_water.contains(&2) && !native_water.contains(&3),
            "26.3 客户端 fluid 标签应保持数据集 id：{native_water:?}"
        );
    }
}
