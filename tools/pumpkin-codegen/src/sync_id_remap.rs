use std::fs;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

use crate::registry::SYNCED_REGISTRIES;
use crate::remap::version_patterns;
use crate::version::JavaMinecraftVersion;

/// The version whose datapack folder defines the server's native runtime id
/// space (matching `NATIVE_DATA_VERSION` in pumpkin-data).
const NATIVE_FOLDER: &str = "26_3";

/// Versions with their own synced-registry datapack folders, ascending.
/// `(folder, ident)` — mirrors the table in `registry.rs`.
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
        _ => unreachable!("unmapped version folder {ident_str}"),
    }
}

/// Stand-in entries for dataset ids that a client version's registry lacks.
/// The client can never render the true entry, so the closest vanilla
/// analogue is used.
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

/// Default stand-in per registry when a missing id has no explicit fallback.
const DEFAULT_FALLBACKS: &[(&str, &str)] =
    &[("worldgen/biome", "plains"), ("damage_type", "generic")];

/// Registry path → snake_case identifier fragment used for generated names
/// (`worldgen/biome` → `biome`).
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

/// Generates per-version tables translating the server's native (26.3)
/// synced-registry ids into the id space of the registry data actually synced
/// to that client version, plus `remap_*_id_for_version` dispatch functions.
///
/// Synced registries (biome, damage_type, ...) are dynamic registries: the
/// client builds them from the entries the server sends at configuration time,
/// with the server's per-version table order. Runtime data (chunk biome
/// palettes, damage events, ...) references native dataset ids, which diverge
/// from a client version's registry set once entries are added between
/// versions.
pub(crate) fn build() -> TokenStream {
    let mut static_values = TokenStream::new();
    // (fn fragment, accumulated match arms)
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
        use pumpkin_util::version::JavaMinecraftVersion;

        #static_values

        #functions
    }
}
