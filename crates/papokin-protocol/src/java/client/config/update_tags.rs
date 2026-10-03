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
        RegistryKey::GameEvent => {
            papokin_data::game_event_id_remap::remap_game_event_id_for_version(id, version)
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

    /// `game_event` 标签对无逐版本真值的旧客户端（1.17–1.21.9、26.1）
    /// 整体省略（客户端保留内建原版标签）；1.21.11 有经 Papo 实抓验证
    /// 的闭式重映射规则，按 1.21.11 注册序下发。fluid 注册序
    /// 1.13–26.x 与数据集同序，标签 id 必须原样下发——客户端的眼睛
    /// 入液判定走 FluidTags，lava 标签一旦含水、water 标签不含水，
    /// 浸水就会渲染岩浆红屏且没有水下效果（2026-10-01 勘误回归锚点）。
    #[test]
    fn game_event_era_gates_and_fluid_ids_unchanged_for_old_clients() {
        let keys = [
            RegistryKey::Block,
            RegistryKey::Item,
            RegistryKey::Fluid,
            RegistryKey::EntityType,
            RegistryKey::GameEvent,
        ];

        let old = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_21_9,
        ));
        assert!(
            !old.contains_key("minecraft:game_event"),
            "1.21.9 客户端不应收到 game_event 标签：{:?}",
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

        // 1.21.11：26.3 空间 id 经闭式规则换算（≤8 恒等；9/32 丢弃；
        // 其余 −1），与 Papo 1.21.11 实抓真值全等。
        let v1_21_11 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_21_11,
        ));
        let game_event = &v1_21_11["minecraft:game_event"];
        assert_eq!(
            game_event["minecraft:shrieker_can_listen"],
            &[37],
            "sculk_sensor_tendrils_clicking 26.3=38 应映射为 1.21.11=37"
        );
        assert_eq!(
            game_event["minecraft:allay_can_listen"],
            &[33],
            "note_block_play 26.3=34 应映射为 1.21.11=33"
        );
        assert_eq!(
            game_event["minecraft:ignore_vibrations_sneaking"],
            &[26, 36, 41, 42, 29, 28],
            "hit_ground/projectile_shoot/step/swim/item_interact_start/finish 应各 −1"
        );
        // warden_can_listen 全表逐 id 比对（1.21.11 空间真值：
        // 数据集 warden 列表（不含 flap）+ shriek(39) + tendrils(37)）
        assert_eq!(
            game_event["minecraft:warden_can_listen"],
            &[
                1, 2, 3, 5, 6, 7, 8, 0, 4, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                24, 25, 26, 27, 28, 32, 33, 34, 35, 36, 38, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
                50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 39, 37,
            ],
            "warden_can_listen 必须逐 id 落在 1.21.11 注册序"
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

    /// 同步类注册表的标签 id 必须落在目标客户端「登录同步表」的 id
    /// 空间（各版本数据包文件夹字母序），而不是数据集（26.3）空间：
    /// 26.3 新增群系按字母序插在中段，两个空间的同名群系 id 大面积
    /// 不同（1.21.11 的 65 个共享群系中 57 个错位——2026-10-01 勘误，
    /// 此前生成器只按 26.3 文件夹解析全部版本的标签 id）。
    #[test]
    fn synced_registry_tag_ids_match_per_version_space() {
        let biome_keys = [RegistryKey::WorldgenBiome];
        let parse =
            |version| parse_all_registries(&serialize(&CUpdateTags::new(&biome_keys), version));

        let old = parse(JavaMinecraftVersion::V_1_21_11);
        // 1_21_11 文件夹字母序下的 is_ocean（deep_* 经 #is_deep_ocean 展开）
        assert_eq!(
            old["minecraft:worldgen/biome"]["minecraft:is_ocean"],
            &[11, 9, 13, 12, 22, 35, 6, 29, 58],
            "1.21.11 群系标签 id 必须落在该版本同步表空间"
        );

        let native = parse(JavaMinecraftVersion::V_26_3);
        assert_eq!(
            native["minecraft:worldgen/biome"]["minecraft:is_ocean"],
            &[12, 10, 14, 13, 23, 36, 6, 30, 60],
            "26.3 原生客户端保持数据集（26_3 文件夹）id 空间"
        );
    }

    /// `damage_type` 自 1.19.4 起由本服同步注册表，1.19.4 客户端必须
    /// 收到配套标签，否则其内建标签会相对同步后的注册表错位失效；
    /// 1.19.3 及更早客户端不省略（彼版本无此注册表，有效性门控拦截）。
    #[test]
    fn damage_type_tags_start_at_1_19_4_in_own_id_space() {
        let keys = [RegistryKey::DamageType];

        let modern = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_19_4,
        ));
        // 1_20 数据包文件夹字母序下 arrow=0、wind_charge=45 等
        assert_eq!(
            modern["minecraft:damage_type"]["minecraft:is_projectile"],
            &[0, 38, 26, 39, 12, 41, 37],
            "1.19.4 的 damage_type 标签 id 必须落在 1_20 文件夹空间"
        );

        let ancient = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_19_3,
        ));
        assert!(
            !ancient.contains_key("minecraft:damage_type"),
            "1.19.3 客户端不应收到 damage_type 标签"
        );
    }

    /// 冻结注册表时代的组合整体省略（客户端保留内建标签），同步
    /// 时代起才下发；`game_event` 在 26.x 家族内部亦有漂移：26.2 的
    /// bounce 插在 26.3 序第 9 位，26.1 客户端整体错位，须一并省略；
    /// 1.21.11 有验证过的闭式重映射，正常下发。兴趣点注册序自 1.19
    /// 起跨版本稳定（17 项），全版本下发。
    #[test]
    fn frozen_era_registries_are_omitted_until_synced() {
        let keys = [
            RegistryKey::BannerPattern,
            RegistryKey::PaintingVariant,
            RegistryKey::Instrument,
            RegistryKey::PointOfInterestType,
            RegistryKey::CatVariant,
            RegistryKey::GameEvent,
        ];

        let v1_19_4 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_19_4,
        ));
        assert!(!v1_19_4.contains_key("minecraft:banner_pattern"));
        assert!(!v1_19_4.contains_key("minecraft:painting_variant"));
        assert!(!v1_19_4.contains_key("minecraft:instrument"));
        assert!(v1_19_4.contains_key("minecraft:point_of_interest_type"));
        assert!(!v1_19_4.contains_key("minecraft:game_event"));

        let v1_20_5 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_20_5,
        ));
        assert!(v1_20_5.contains_key("minecraft:banner_pattern"));
        assert!(v1_20_5.contains_key("minecraft:painting_variant"));
        assert!(!v1_20_5.contains_key("minecraft:instrument"));

        let v1_21_2 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_21_2,
        ));
        assert!(v1_21_2.contains_key("minecraft:instrument"));
        // 兴趣点：17 项注册序跨版本稳定，全版本下发；猫变种：任何
        // 版本都无逐版本真值，全部省略。
        for regs in [&v1_19_4, &v1_20_5, &v1_21_2] {
            assert!(regs.contains_key("minecraft:point_of_interest_type"));
            assert!(!regs.contains_key("minecraft:cat_variant"));
        }
        // 兴趣点标签 id 真值（Papo 1.21.11 实抓）：bee_home=[15,16]、
        // village=[0..=14]、acquirable_job_site=[0..=12]
        let v1_21_11 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_1_21_11,
        ));
        let poi = &v1_21_11["minecraft:point_of_interest_type"];
        assert_eq!(poi["minecraft:bee_home"], &[15, 16]);
        assert_eq!(
            poi["minecraft:village"],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
        );
        assert_eq!(
            poi["minecraft:acquirable_job_site"],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );

        // game_event：26.1 省略（bounce 插入位导致整体偏移）、1.21.11
        // 有闭式重映射、26.2 起与数据集同集合下发。
        let v26_1 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_26_1,
        ));
        assert!(
            !v26_1.contains_key("minecraft:game_event"),
            "26.1 客户端不应收到 game_event 标签"
        );
        assert!(
            v1_21_11.contains_key("minecraft:game_event"),
            "1.21.11 客户端应收到经重映射的 game_event 标签"
        );
        let v26_2 = parse_all_registries(&serialize(
            &CUpdateTags::new(&keys),
            JavaMinecraftVersion::V_26_2,
        ));
        assert!(
            v26_2.contains_key("minecraft:game_event"),
            "26.2 与 26.3 事件集合一致，应正常下发"
        );
    }
}
