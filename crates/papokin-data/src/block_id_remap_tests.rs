//! `block_id_remap` 生成表的回归测试。
//!
//! 锚点期望值来自 ViaVersion 状态重映射组合与各版本冻结注册表的
//! 独立推导（见 note/15 热修章节）：原版按方块注册序连续分配全局
//! 状态 id，因此数据集方块在目标版本状态空间中的最小映射状态的
//! 排名即其目标版本方块 id。

use papokin_util::version::JavaMinecraftVersion;

use crate::block_id_remap::{
    self, BLOCK_ID_REMAP_V_26_3_TO_V_1_13, BLOCK_ID_REMAP_V_26_3_TO_V_1_16,
    BLOCK_ID_REMAP_V_26_3_TO_V_1_19_4, BLOCK_ID_REMAP_V_26_3_TO_V_1_21_11,
    BLOCK_ID_REMAP_V_26_3_TO_V_26_2,
};

/// 断言 `(数据集 id, 名称, 期望目标 id)` 锚点列表。
fn assert_anchors(table: &[u16], anchors: &[(u16, &str, u16)]) {
    for &(ds_id, name, expected) in anchors {
        let got = table[usize::from(ds_id)];
        assert_eq!(
            got, expected,
            "方块 {name}（数据集 id {ds_id}）应映射为 {expected}，实际 {got}"
        );
    }
}

/// 1.21.11（协议目标版本）锚点：早期方块恒等、26.x 插入导致的
/// 位移、后期方块与数据集独有方块（映射 0=空气）。
#[test]
fn anchors_v1_21_11() {
    assert_anchors(
        BLOCK_ID_REMAP_V_26_3_TO_V_1_21_11,
        &[
            (1, "stone", 1),
            (9, "dirt", 9),
            (13, "oak_planks", 13),
            (51, "oak_log", 49),
            (265, "oak_door", 219),
            (250, "crafting_table", 205),
            (253, "furnace", 208),
            (237, "obsidian", 192),
            (247, "diamond_ore", 202),
            (118, "note_block", 109),
            (1241, "deepslate", 1121),
            (1007, "crying_obsidian", 915),
            (58, "pale_oak_log", 56),
            (1070, "amethyst_cluster", 978),
            // 26.x 独有方块：目标版本不存在 → 空气
            (1097, "sulfur_bricks", 0),
            (60, "poplar_log", 0),
        ],
    );
}

/// 1.19.4 锚点：实验性数据包注册的 torchflower_crop（3 态）介于
/// end_stone_bricks 与 beetroots 之间，推导时其多出的 age=2 状态
/// 按状态数增长吸收，torchflower_crop 与后续方块排名不受影响。
#[test]
fn anchors_v1_19_4_experimental_absorb() {
    assert_anchors(
        BLOCK_ID_REMAP_V_26_3_TO_V_1_19_4,
        &[
            (719, "end_stone_bricks", 596),
            (720, "torchflower_crop", 597),
            (723, "beetroots", 598),
            (724, "dirt_path", 599),
        ],
    );
}

/// 1.13 锚点：早期版本位移、改名方块（short_grass ← grass）映射到
/// 改名前的注册表位置、后期方块缺席；stripped_oak_log 落在其余
/// 剥皮原木之后是原版 1.13 注册序的固有怪癖，作强锚点。
#[test]
fn anchors_v1_13() {
    assert_anchors(
        BLOCK_ID_REMAP_V_26_3_TO_V_1_13,
        &[
            (1, "stone", 1),
            (51, "oak_log", 34),
            (71, "stripped_oak_log", 45),
            (94, "oak_leaves", 58),
            (140, "short_grass", 94),
            (1241, "deepslate", 0),
            (1007, "crying_obsidian", 0),
        ],
    );
}

/// 1.16 与 26.2 的抽样锚点（1.16 引入 crying_obsidian 并在 34 号位
/// 插入 nether_gold_ore 顶后 oak_log；26.2 仍含 sulfur 系列而 26.3
/// 的 poplar 独有）。
#[test]
fn anchors_v1_16_and_v26_2() {
    assert_anchors(
        BLOCK_ID_REMAP_V_26_3_TO_V_1_16,
        &[
            (50, "nether_gold_ore", 34),
            (51, "oak_log", 35),
            (718, "purpur_stairs", 496),
            (1007, "crying_obsidian", 737),
            (1241, "deepslate", 0),
        ],
    );
    assert_anchors(
        BLOCK_ID_REMAP_V_26_3_TO_V_26_2,
        &[
            (51, "oak_log", 49),
            (665, "stone_slab", 610),
            (1097, "sulfur_bricks", 1007),
            (60, "poplar_log", 0),
        ],
    );
}

/// 碰撞回归锚点：26.x 独有方块的 ViaVersion 回退会落在原版台阶/
/// 楼梯等替身方块的真实区间上（共享 (min,count)），裁决层依据
/// 版本数据包标签集保住原版方块、把 26.x 独有映射清零。
#[test]
fn anchors_collision_fallbacks_prefer_vanilla() {
    assert_anchors(
        BLOCK_ID_REMAP_V_26_3_TO_V_1_21_11,
        &[
            (654, "spruce_slab", 598),
            (658, "cherry_slab", 602),
            (972, "crimson_roots", 880),
        ],
    );
}

/// 全部版本表的单射性：除映射到 0（空气）的缺席方块外，在场方块的
/// 目标 id 互不相同（排名不重复）。
#[test]
fn all_tables_are_injective() {
    let tables: &[&[u16]] = &[
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_13,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_13_2,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_14,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_15,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_16,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_16_2,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_17,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_18,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_19,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_19_3,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_19_4,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_20,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_20_2,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_20_3,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_20_5,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_2,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_4,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_5,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_6,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_7,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_9,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_1_21_11,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_26_1,
        block_id_remap::BLOCK_ID_REMAP_V_26_3_TO_V_26_2,
    ];
    for (i, table) in tables.iter().enumerate() {
        let mut seen = std::collections::HashSet::with_capacity(table.len());
        let mut distinct = 0;
        for &id in *table {
            if id != 0 {
                seen.insert(id);
                distinct += 1;
            }
        }
        assert_eq!(
            seen.len(),
            distinct,
            "第 {i} 张表存在重复的目标方块 id，排名单射性被破坏"
        );
    }
}

/// 分发函数：协议版本走对应表；原生版本与未知 id 恒等返回。
#[test]
fn dispatch_by_version() {
    use block_id_remap::remap_block_id_for_version as remap;
    // oak_log：数据集 id 51 → 1.21.11 的 49
    assert_eq!(remap(51, JavaMinecraftVersion::V_1_21_11), 49);
    // 协议别名版本与同表版本一致
    assert_eq!(
        remap(51, JavaMinecraftVersion::V_1_14_4),
        remap(51, JavaMinecraftVersion::V_1_14)
    );
    // 原生版本（26.3）不在表内：恒等
    assert_eq!(remap(51, JavaMinecraftVersion::V_26_3), 51);
    // 超出表长的 id：恒等返回（与状态重映射的缺省约定一致）
    assert_eq!(remap(u16::MAX, JavaMinecraftVersion::V_1_21_11), u16::MAX);
}
