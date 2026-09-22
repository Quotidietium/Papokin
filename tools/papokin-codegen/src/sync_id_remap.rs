use std::fs;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

use crate::registry::SYNCED_REGISTRIES;
use crate::remap::version_patterns;
use crate::version::JavaMinecraftVersion;

/// 其 datapack 文件夹定义了服务器原生运行时 id 的版本
/// 空间（与 papokin-data 中的 `NATIVE_DATA_VERSION` 一致）。
const NATIVE_FOLDER: &str = "26_3";

/// 拥有各自同步注册表数据包文件夹的版本，按升序。
/// `(folder, ident)`——与 `registry.rs` 中的表对应。
const VERSIONS: &[(&str, &str)] = &[
    ("1_16", "V_1_16"),
    ("1_16_2", "V_1_16_2"),
    ("1_17", "V_1_17"),
    ("1_18", "V_1_18"),
    ("1_19", "V_1_19"),
    ("1_20", "V_1_20"),
    ("1_20_2", "V_1_20_2"),
    ("1_21", "V_1_21"),
    ("1_21_2", "V_1_21_2"),
    ("1_21_4", "V_1_21_4"),
    ("1_21_5", "V_1_21_5"),
    ("1_21_6", "V_1_21_6"),
    ("1_21_7", "V_1_21_7"),
    ("1_21_9", "V_1_21_9"),
    ("1_21_11", "V_1_21_11"),
    ("26_1", "V_26_1"),
    ("26_2", "V_26_2"),
];

fn version_enum(ident_str: &str) -> JavaMinecraftVersion {
    match ident_str {
        "1_16" => JavaMinecraftVersion::V_1_16,
        "1_16_2" => JavaMinecraftVersion::V_1_16_2,
        "1_17" => JavaMinecraftVersion::V_1_17,
        "1_18" => JavaMinecraftVersion::V_1_18,
        "1_19" => JavaMinecraftVersion::V_1_19,
        "1_20" => JavaMinecraftVersion::V_1_20,
        "1_20_2" => JavaMinecraftVersion::V_1_20_2,
        "1_21" => JavaMinecraftVersion::V_1_21,
        "1_21_2" => JavaMinecraftVersion::V_1_21_2,
        "1_21_4" => JavaMinecraftVersion::V_1_21_4,
        "1_21_5" => JavaMinecraftVersion::V_1_21_5,
        "1_21_6" => JavaMinecraftVersion::V_1_21_6,
        "1_21_7" => JavaMinecraftVersion::V_1_21_7,
        "1_21_9" => JavaMinecraftVersion::V_1_21_9,
        "1_21_11" => JavaMinecraftVersion::V_1_21_11,
        "26_1" => JavaMinecraftVersion::V_26_1,
        "26_2" => JavaMinecraftVersion::V_26_2,
        _ => unreachable!("未映射的版本文件夹 {ident_str}"),
    }
}

/// 作为占位条目，用于填补某客户端版本注册表中缺失的数据集 ID。
/// 客户端永远无法渲染真实的条目，因此最接近原版的
/// 则使用对应的类似实现。
const FALLBACKS: &[(&str, &[(&str, &str)])] = &[
    (
        "worldgen/biome",
        &[
            ("dappled_forest", "forest"),
            ("sulfur_caves", "dripstone_caves"),
        ],
    ),
    ("damage_type", &[("sulfur_cube_hot", "hot_floor")]),
    ("jukebox_song", &[("bounce", "cat")]),
];

/// 当缺失 ID 没有显式回退时，各注册表使用的默认替身值。
const DEFAULT_FALLBACKS: &[(&str, &str)] =
    &[("worldgen/biome", "plains"), ("damage_type", "generic")];

/// 注册表路径 → 用于生成名称的 snake_case 标识符片段
/// (`worldgen/biome` → `biome`)。
fn fn_fragment(registry: &str) -> &str {
    registry.strip_prefix("worldgen/").unwrap_or(registry)
}

fn sorted_entry_names(version_folder: &str, registry: &str) -> Option<Vec<String>> {
    let dir = std::path::Path::new("../../assets/datapacks")
        .join(version_folder)
        .join("data/minecraft")
        .join(registry);
    if !dir.is_dir() {
        return None;
    }
    let mut names: Vec<String> = fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    Some(names)
}

/// 生成各版本的对照表，用于转换服务器原生的（26.3）
/// 已同步注册表的 id 映射到实际同步的注册表数据的 id 空间
/// 重映射到该客户端版本，外加 `remap_*_id_for_version` 分发函数。
///
/// 同步注册表（biome、damage_type 等）是动态注册表：
/// 客户端会根据服务器在配置阶段发送的条目构建它们，
/// 与服务器按版本的表顺序一致。运行时数据（区块生物群系
/// 调色板、伤害事件等）引用原生数据集 id，而这些 id 会偏离
/// 一旦在版本之间添加了条目，就不会偏离客户端版本的注册表集
/// 版本。
pub(crate) fn build() -> TokenStream {
    let mut static_values = TokenStream::new();
    // （fn 片段，累积的 match 分支）
    let mut registry_arms: Vec<(&str, TokenStream)> = Vec::new();

    for &(folder, ident_str) in VERSIONS {
        if folder == NATIVE_FOLDER {
            continue;
        }

        for &registry in SYNCED_REGISTRIES {
            let (Some(native_names), Some(version_names)) = (
                sorted_entry_names(NATIVE_FOLDER, registry),
                sorted_entry_names(folder, registry),
            ) else {
                continue;
            };
            if native_names == version_names {
                continue;
            }

            let overrides = FALLBACKS
                .iter()
                .find(|(reg, _)| *reg == registry)
                .map(|(_, overrides)| *overrides)
                .unwrap_or(&[]);
            let default_name = DEFAULT_FALLBACKS
                .iter()
                .find(|(reg, _)| *reg == registry)
                .map(|(_, name)| *name);

            let mut table = Vec::with_capacity(native_names.len());
            let mut is_identity = true;
            for (native_index, name) in native_names.iter().enumerate() {
                let target = version_names
                    .iter()
                    .position(|candidate| candidate == name)
                    .or_else(|| {
                        overrides
                            .iter()
                            .find(|(from, _)| from == name)
                            .and_then(|(_, to)| {
                                version_names.iter().position(|candidate| candidate == to)
                            })
                    })
                    .or_else(|| {
                        default_name.and_then(|name| {
                            version_names.iter().position(|candidate| candidate == name)
                        })
                    })
                    .unwrap_or(0);
                is_identity &= target == native_index;
                table.push(u16::try_from(target).unwrap_or(0));
            }
            if is_identity {
                continue;
            }

            let fragment = fn_fragment(registry);
            let static_ident = format_ident!(
                "{}_SYNC_REMAP_V_{}_TO_{}",
                fragment.to_uppercase(),
                NATIVE_FOLDER.to_uppercase(),
                ident_str
            );
            let table_tokens: Vec<_> = table
                .iter()
                .map(|&id| Literal::u16_unsuffixed(id))
                .collect();
            static_values.extend(quote! {
                pub static #static_ident: &[u16] = &[#(#table_tokens),*];
            });

            let patterns = version_patterns(version_enum(folder));
            let arg_ident = format_ident!("{fragment}");
            let arm = quote! {
                #(#patterns)|* => #static_ident
                    .get(usize::from(#arg_ident))
                    .copied()
                    .unwrap_or(#arg_ident),
            };
            match registry_arms.iter_mut().find(|(name, _)| *name == fragment) {
                Some((_, arms)) => arms.extend(arm),
                None => {
                    registry_arms.push((fragment, arm));
                }
            }
        }
    }

    let mut functions = TokenStream::new();
    for (fragment, arms) in registry_arms {
        let fn_ident = format_ident!("remap_{fragment}_id_for_version");
        let arg_ident = format_ident!("{fragment}");
        functions.extend(quote! {
            #[must_use]
            pub fn #fn_ident(#arg_ident: u16, version: JavaMinecraftVersion) -> u16 {
                match version {
                    #arms
                    _ => #arg_ident,
                }
            }
        });
    }

    quote! {
        use papokin_util::version::JavaMinecraftVersion;

        #static_values

        #functions
    }
}
