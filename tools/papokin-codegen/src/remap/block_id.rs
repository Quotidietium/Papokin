use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

use crate::block::BlockAssets;
use crate::remap::{MappingNode, ParsedMappings, Remapper};
use crate::remap_nodes;
use crate::version::JavaMinecraftVersion;

/// 连续区间条目：一个数据集方块在目标版本状态空间中的去重映射集合。
struct Entry<'a> {
    /// 区间起点（最小映射状态）。
    min: u16,
    /// 去重后的状态数（区间长度）。
    count: usize,
    /// 数据集原始状态数（双射性判定用：在场方块两者相等）。
    states: usize,
    /// 数据集方块下标。
    idx: usize,
    /// 数据集方块 id。
    id: u16,
    /// 数据集方块名。
    name: &'a str,
}

/// 见 [`Entry`]；`'a` 为数据集方块表的生命周期。

/// 把每个数据集方块的全部状态映射到目标状态空间并去重，仅保留
/// 连续区间（散布回退 span > count 的方块目标版本不存在）。
fn build_entries<'a>(state_map: &[u16], blocks: &'a [(u16, &str, Vec<u16>)]) -> Vec<Entry<'a>> {
    let mut entries: Vec<Entry> = Vec::with_capacity(blocks.len());
    for (idx, (id, name, states)) in blocks.iter().enumerate() {
        let mut unique: Vec<u16> = states
            .iter()
            .map(|&s| state_map.get(usize::from(s)).copied().unwrap_or(0))
            .collect();
        unique.sort_unstable();
        unique.dedup();
        let count = unique.len();
        let span = u32::from(unique.last().copied().unwrap_or(0))
            - u32::from(unique.first().copied().unwrap_or(0))
            + 1;
        if u32::try_from(count).is_ok_and(|c| c == span) {
            entries.push(Entry {
                min: unique.first().copied().unwrap_or(0),
                count,
                states: states.len(),
                idx,
                id: *id,
                name,
            });
        }
        // 去重集合不连续（span > count）：散布回退，目标版本不存在。
    }
    entries.sort_by(|a, b| {
        a.min
            .cmp(&b.min)
            .then(b.count.cmp(&a.count))
            .then(a.id.cmp(&b.id))
    });
    entries
}

/// 从目标版本的状态映射推导数据集方块 id → 目标版本方块 id。
///
/// 原版按方块注册序连续分配全局状态 id，因此数据集方块在目标版本
/// 状态空间中的区间排名即其目标版本方块 id。区间平铺扫描，同一起点
/// 出现多个候选（**区间碰撞**）时按四层优先级裁决，见 [`pick_winner`]。
///
/// 光标与下一区间之间的洞：紧随已接受方块的洞按该方块的**状态数
/// 增长**吸收（排名不变；实例：1.19.4 实验性数据包的 torchflower_crop
/// 有 3 态，1.20 正式版收缩为 2 态，多出的 age=2 状态成为洞）；起点处
/// 的洞无法归因，panic。
///
/// `label` 仅用于诊断输出（版本名或“恒等自检”）。
fn derive_block_ids(
    state_map: &[u16],
    blocks: &[(u16, &str, Vec<u16>)],
    label: &str,
    oracle: &dyn Fn(&[Entry]) -> usize,
) -> Vec<u16> {
    let entries = build_entries(state_map, blocks);

    let mut ids = vec![0u16; blocks.len()];
    let mut cursor = 0u32;
    let mut ranked = 0u16;
    let mut accepted_any = false;
    let mut i = 0;
    while i < entries.len() {
        let min32 = u32::from(entries[i].min);
        if min32 > cursor {
            if accepted_any {
                let gap = min32 - cursor;
                eprintln!(
                    "警告[{label}]：目标状态 {cursor}..{min32}（方块 {} 之前，{gap} 态）吸收为上一方块的状态数增长",
                    entries[i].name
                );
            } else {
                panic!(
                    "方块 id 重映射覆盖自检失败[{label}]：目标状态 0..{min32}（方块 {} 之前）未被任何数据集方块命中",
                    entries[i].name
                );
            }
            cursor = min32;
        }

        // 收集同一起点的候选组（区间碰撞），由预言机裁决胜者。
        let mut end = i + 1;
        while end < entries.len() && u32::from(entries[end].min) == min32 {
            end += 1;
        }
        if min32 < cursor {
            // 整组落在已接受区间内部：全部为回退（真实方块区间互不
            // 重叠）→ 组内成员缺席。
            i = end;
            continue;
        }
        let winner = i + oracle(&entries[i..end]);
        let entry = &entries[winner];
        ids[entry.idx] = ranked;
        ranked = ranked.checked_add(1).expect("目标版本方块数超出 u16");
        cursor += entry.count as u32;
        accepted_any = true;
        i = end;
    }

    let target_max = u32::from(state_map.iter().copied().max().unwrap_or(0));
    assert_eq!(
        cursor,
        target_max + 1,
        "方块 id 重映射平铺自检失败[{label}]：区间终点 {cursor} != 目标状态空间末尾 {}",
        target_max + 1
    );

    ids
}

/// 加载某版本数据包全部方块标签中出现过的名字集（含展开后的嵌套
/// 标签成员）。数据集独有方块（26.x 新增）不可能出现在旧版本数据
/// 包里，因此该集合可用作“方块存在于该版本”的在裁决信号。
fn block_tag_names(version: JavaMinecraftVersion) -> std::collections::HashSet<String> {
    let folder = format!("{version:?}")
        .trim_start_matches("V_")
        .to_lowercase();
    let data_dir = std::path::Path::new("../../assets/datapacks")
        .join(&folder)
        .join("data");
    crate::tag::load_datapack_tags(&data_dir)
        .into_iter()
        .find(|(cat, _)| cat == "block")
        .map_or_else(std::collections::HashSet::new, |(_, tag_map)| {
            tag_map
                .into_values()
                .flat_map(|names| names.into_iter())
                .collect()
        })
}

/// 生成各版本方块 ID 重映射表及 `remap_block_id_for_version` 函数。
///
/// ViaVersion 映射只提供方块**状态** id 的跨版本转换，没有方块注册表
/// id 区段。方块 id 由“状态按注册序连续分配”不变式经区间平铺推导。
///
/// 数据集（26.3）独有方块的回退可能落在替身方块的真实区间上，与
/// 替身共享整段区间（同一起点多候选）。裁决按四层优先级（见生成
/// 时注释与 note/15）：本版标签在场 > 任一版本独占区间 > 非 26.x
/// 独有（以 1.21.11——其碰撞全部可由标签裁决——的推导结果为准）>
/// 数据集 id 最小。原版注册表偶有跨版本重排（如 26.3 移动了
/// smooth_stone_slab），故不做全局单调性假设。
///
/// 生成时自检（失败即 panic，保证表不可静默漂移）：
/// - 恒等自检：对数据集自身的状态布局做同一推导，排名必须等于
///   方块 id（验证不变式在数据集中成立）；
/// - 平铺自检：接受区间恰好覆盖目标状态空间，无重叠无缝隙。
pub fn build() -> TokenStream {
    let remapper: Remapper<_, Option<Vec<u16>>> = Remapper {
        version: JavaMinecraftVersion::V_26_3,
        remapper: |first, second| match (first, second) {
            (Some(first), Some(second)) => Some(
                first
                    .iter()
                    .map(|id| second.get(usize::from(*id)).copied().unwrap_or(0))
                    .collect(),
            ),
            (None, Some(second)) => Some(
                (0..second.len())
                    .map(|id| second.get(id).copied().unwrap_or(0))
                    .collect(),
            ),
            (Some(first), None) => Some(first.clone()),
            _ => None,
        },
        serializer: |&file| {
            ParsedMappings::parse_mapping_file(file, "blockstates")
                .map(|mappings| mappings.to_u16(file))
        },
    };

    let all_state_mappings = remap_nodes!(remapper);

    let assets: BlockAssets =
        serde_json::from_str(&std::fs::read_to_string("../../assets/blocks.json").unwrap())
            .expect("解析 blocks.json 失败");
    // 按方块 id 排序的 (id, 名称, 状态列表)，是推导的输入。
    let mut blocks: Vec<(u16, &str, Vec<u16>)> = assets
        .blocks
        .iter()
        .map(|b| {
            (
                b.id.0,
                b.name.as_str(),
                b.states.iter().map(|s| s.id.0).collect(),
            )
        })
        .collect();
    blocks.sort_by_key(|(id, _, _)| *id);

    // 恒等自检：数据集自身的状态布局必须满足“按最小状态排名 == 方块 id”。
    let max_state = blocks
        .iter()
        .flat_map(|(_, _, states)| states.iter().copied())
        .max()
        .expect("blocks.json 无方块状态");
    let identity: Vec<u16> = (0..=max_state).collect();
    let identity_ids = derive_block_ids(&identity, &blocks, "恒等自检", &|cands| {
        // 恒等布局无碰撞；取唯一候选。
        cands
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.id)
            .map(|(k, _)| k)
            .unwrap_or(0)
    });
    for (rank, (id, block)) in identity_ids.iter().zip(blocks.iter()).enumerate() {
        let (_, name, _) = block;
        assert_eq!(
            rank, *id as usize,
            "恒等自检失败：方块 {name}（id {id}）按最小状态排名为 {rank}"
        );
    }

    // ── 裁决信号的准备 ──────────────────────────────────────────
    // 1) 每版本标签名字集；
    // 2) 全局“独占”集：在至少一个版本的平铺中独占区间的方块（结构性
    //    在场证据）；
    // 3) 26.x 独有集：以 1.21.11（碰撞全部可由标签裁决）的推导结果中
    //    映射为 0 的方块为准。
    let versioned: Vec<(
        JavaMinecraftVersion,
        Vec<u16>,
        std::collections::HashSet<String>,
    )> = all_state_mappings
        .iter()
        .filter_map(|(ver, mapping)| {
            if (*ver as u32) < (JavaMinecraftVersion::V_1_13 as u32) {
                return None; // 方块标签只对 1.13+ 客户端下发
            }
            mapping
                .as_ref()
                .map(|m| (*ver, m.clone(), block_tag_names(*ver)))
        })
        .collect();

    let mut sole_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_, state_map, _) in &versioned {
        for group in group_by_min(&build_entries(state_map, &blocks)) {
            if group.len() == 1 {
                sole_names.insert(group[0].name.to_string());
            }
        }
    }

    let only_1_21_11: Vec<(
        JavaMinecraftVersion,
        Vec<u16>,
        std::collections::HashSet<String>,
    )> = versioned
        .iter()
        .filter(|(ver, _, _)| *ver == JavaMinecraftVersion::V_1_21_11)
        .cloned()
        .collect();
    let only_1_21_11 = only_1_21_11.first().expect("状态映射缺少 1.21.11");
    let tags_1_21_11 = &only_1_21_11.2;
    let ids_1_21_11 = derive_block_ids(
        &only_1_21_11.1,
        &blocks,
        "V_1_21_11",
        &make_oracle(tags_1_21_11, &sole_names, &std::collections::HashSet::new()),
    );
    let only26: std::collections::HashSet<String> = blocks
        .iter()
        .zip(ids_1_21_11.iter())
        .filter(|((id, _, _), mapped)| *id != 0 && **mapped == 0)
        .map(|((_, name, _), _)| (*name).to_string())
        .collect();

    // ── 全版本推导 ──────────────────────────────────────────────
    let mut static_values = TokenStream::new();
    let mut match_arms = TokenStream::new();

    for (ver, state_mapping, tags) in &versioned {
        let oracle = make_oracle(tags, &sole_names, &only26);
        let ids = derive_block_ids(state_mapping, &blocks, &format!("{ver:?}"), &oracle);

        let ident = format_ident!(
            "{}",
            format!("BLOCK_ID_REMAP_{:?}_TO_{:?}", remapper.version, ver).to_uppercase()
        );
        let mapping_tokens: Vec<_> = ids.iter().copied().map(Literal::u16_unsuffixed).collect();
        static_values.extend(quote! {
            pub static #ident: &[u16] = &[#(#mapping_tokens),*];
        });
        let versions = crate::remap::version_patterns(*ver);
        match_arms.extend(quote! {
            #(#versions)|* => #ident
                .get(usize::from(block_id))
                .copied()
                .unwrap_or(block_id),
        });
    }

    quote! {
        use papokin_util::version::JavaMinecraftVersion;

        #static_values

        /// 将内置数据集（26.3）的方块 id 重映射为 `version` 客户端
        /// 冻结方块注册表中的 id。数据集独有、目标版本不存在的方块
        /// 映射到 0（空气）。原生版本（26.3）与未知版本恒等返回。
        #[must_use]
        pub fn remap_block_id_for_version(block_id: u16, version: JavaMinecraftVersion) -> u16 {
            match version {
                #match_arms
                _ => block_id,
            }
        }
    }
}

/// 把条目按区间起点分组，返回按起点升序的组列表。
fn group_by_min<'a, 'b>(entries: &'b [Entry<'a>]) -> Vec<&'b [Entry<'a>]> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let mut end = i + 1;
        while end < entries.len() && entries[end].min == entries[i].min {
            end += 1;
        }
        groups.push(&entries[i..end]);
        i = end;
    }
    groups
}

/// 构造区间碰撞的裁决闭包：五层优先级逐层过滤候选组——
/// 1. 名字在该版本的数据包方块标签中（直接在场证据）；
/// 2. 排除“数据集多态方块塌缩成单态”的确定性回退（在场方块的
///    状态映射保持满区间；跨版本状态数增长的真方块如树叶
///    28→14 不受影响。实例：1.13 中 gray_glazed_terracotta
///    （4→4）须胜过 basalt 一族（3→1））；
/// 2. 名字在“至少一个版本独占区间”的全局集合中（结构性在场证据）；
/// 3. 名字不在 26.x 独有集合中（以 1.21.11 的推导结果为准的排除证据）；
/// 4. 数据集 id 最小（最终兜底）。
///
/// 某层过滤后恰好剩一个候选即胜出；剩多个则继续下一层；一层都没
/// 过滤掉任何人时保持原候选集继续。
fn make_oracle(
    tags: &std::collections::HashSet<String>,
    sole_names: &std::collections::HashSet<String>,
    only26: &std::collections::HashSet<String>,
) -> impl Fn(&[Entry]) -> usize {
    /// 按谓词过滤候选下标；过滤结果为空时保持原集合。
    fn retain_if(survivors: &mut Vec<usize>, cands: &[Entry], pred: impl Fn(&Entry) -> bool) {
        let filtered: Vec<usize> = survivors
            .iter()
            .copied()
            .filter(|&k| pred(&cands[k]))
            .collect();
        if !filtered.is_empty() {
            *survivors = filtered;
        }
    }

    move |cands: &[Entry]| -> usize {
        let mut survivors: Vec<usize> = (0..cands.len()).collect();
        retain_if(&mut survivors, cands, |e| tags.contains(e.name));
        retain_if(&mut survivors, cands, |e| !(e.states > 1 && e.count == 1));
        retain_if(&mut survivors, cands, |e| sole_names.contains(e.name));
        retain_if(&mut survivors, cands, |e| !only26.contains(e.name));
        survivors
            .into_iter()
            .min_by_key(|&k| cands[k].id)
            .unwrap_or(0)
    }
}
