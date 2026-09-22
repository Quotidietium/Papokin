#![allow(dead_code, unused)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::empty_structs_with_brackets,
    clippy::semicolon_outside_block,
    clippy::unreachable,
    clippy::undocumented_unsafe_blocks,
    clippy::needless_return,
    clippy::collapsible_if
)]

use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::prelude::*;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

mod advancement;
mod attributes;
mod biome;
mod bitsets;
mod block;
mod block_transformer;
mod carver;
mod chunk_status;
mod chunk_view_lut;
mod composter_increase_chance;
mod configured_feature;
mod damage_type;
mod data_component;
mod dimension;
mod dye_color;
mod effect;
mod enchantments;
mod entity_pose;
mod entity_status;
mod entity_type;
mod environment_attribute;
mod flower_pot_transformations;
mod fluid;
mod fuels;
mod game_event;
mod game_rules;
mod item;
mod jukebox_song;
pub mod loot_table;
mod map_color;
mod map_decoration;
mod material_rule;
mod message_type;
mod meta_data_type;
mod noise_parameter;
mod noise_router;
mod noise_settings;
mod packet;
mod particle;
mod placed_feature;
mod potion;
mod potion_brewing;
mod recipe_remainder;
mod recipes;
mod registry;
mod remap;
mod scoreboard_slot;
mod screen;
mod sdk;
mod sound;
mod sound_category;
mod spawn_egg;
mod statistic;
mod structures;
mod sync_id_remap;
mod tag;
mod tracked_data;
mod translations;
mod trial_spawner;
mod version;
mod villager;
mod wit;
mod world_event;

/// 输出目录，所有生成的 Rust 源文件都写入其中。
pub const OUT_DIR: &str = "../../crates/papokin-data/src/generated";

/// 代码生成器的入口点。并行运行所有已注册的构建函数
/// 并将它们的输出写入 [`OUT_DIR`]。
pub fn main() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::env::set_current_dir(manifest_dir).expect("将当前目录设置为 manifest 目录失败");

    type BuilderFn = fn() -> TokenStream;

    fs::create_dir_all(OUT_DIR).expect("创建输出目录失败");

    let mut build_functions: Vec<(BuilderFn, &str)> = vec![
        (advancement::build, "advancement.rs"),
        (packet::build, "packet.rs"),
        (screen::build, "screen.rs"),
        (particle::build, "particle.rs"),
        (sound::build, "sound.rs"),
        (meta_data_type::build, "meta_data_type.rs"),
        (tracked_data::build, "tracked_data.rs"),
        (chunk_status::build, "chunk_status.rs"),
        (chunk_view_lut::build, "chunk_view_lut.rs"),
        (game_event::build, "game_event.rs"),
        (game_rules::build, "game_rules.rs"),
        (registry::build, "registry.rs"),
        (sync_id_remap::build, "sync_id_remap.rs"),
        (dimension::build, "dimension.rs"),
        (translations::build, "translation.rs"),
        (jukebox_song::build, "jukebox_song.rs"),
        (sound_category::build, "sound_category.rs"),
        (entity_pose::build, "entity_pose.rs"),
        (scoreboard_slot::build, "scoreboard_slot.rs"),
        (world_event::build, "world_event.rs"),
        (entity_type::build, "entity_type.rs"),
        (statistic::build, "statistic.rs"),
        (noise_parameter::build, "noise_parameter.rs"),
        (biome::build, "biome.rs"),
        (damage_type::build, "damage_type.rs"),
        (message_type::build, "message_type.rs"),
        (spawn_egg::build, "spawn_egg.rs"),
        (block::build, "block.rs"),
        (item::build, "item.rs"),
        (structures::build, "structures.rs"),
        (material_rule::build, "material_rule.rs"),
        (noise_settings::build, "noise_settings.rs"),
        (fluid::build, "fluid.rs"),
        (entity_status::build, "entity_status.rs"),
        (tag::build, "tag.rs"),
        (noise_router::build, "noise_router.rs"),
        (villager::build, "villager.rs"),
        (
            flower_pot_transformations::build,
            "flower_pot_transformations.rs",
        ),
        (
            composter_increase_chance::build,
            "composter_increase_chance.rs",
        ),
        (recipes::build, "recipes.rs"),
        (enchantments::build, "enchantment.rs"),
        (fuels::build, "fuels.rs"),
        (data_component::build, "data_component.rs"),
        (attributes::build, "attributes.rs"),
        (environment_attribute::build, "environment_attribute.rs"),
        (effect::build, "effect.rs"),
        (potion::build, "potion.rs"),
        (potion_brewing::build, "potion_brewing.rs"),
        (recipe_remainder::build, "recipe_remainder.rs"),
        (placed_feature::build_enum, "placed_feature.rs"),
        (placed_feature::build, "placed_features_generated.rs"),
        (configured_feature::build_enum, "configured_feature.rs"),
        (
            configured_feature::build,
            "configured_features_generated.rs",
        ),
        (carver::build, "carver.rs"),
        (loot_table::build, "loot_table.rs"),
        (map_color::build, "map_color.rs"),
        (map_decoration::build, "map_decoration.rs"),
        (dye_color::build, "dye_color.rs"),
        (block_transformer::build, "block_transformer.rs"),
        (trial_spawner::build, "trial_spawner.rs"),
    ];
    build_functions.extend(remap::build());

    // 如果给出了任何参数，将其视为文件干名过滤器。
    // 例如 `cargo run -- chest_loot` 仅重新生成 chest_loot.rs。
    let filters: Vec<String> = std::env::args().skip(1).collect();
    let build_functions: Vec<_> = if filters.is_empty() {
        wit::main();
        sdk::main();
        build_functions
    } else {
        build_functions
            .into_iter()
            .filter(|(_, file)| {
                let stem = file.trim_end_matches(".rs");
                filters.iter().any(|f| f == stem || f == *file)
            })
            .collect()
    };

    build_functions.par_iter().for_each(|(build_fn, file)| {
        println!("正在解析 {}", file);

        let raw_code = build_fn().to_string();

        let header = "/* This file is generated. Do not edit manually. */\n";

        let final_code = format_code(&raw_code).map_or_else(
            |_| format!("{header}{raw_code}"),
            |formatted| format!("{header}{formatted}"),
        );

        write_generated_file(&final_code, file);
    });
    println!("完成")
}

/// 将字符串切片转换为由 PascalCase 枚举变体组成的 `TokenStream`。
///
/// # Arguments
/// - `array` – 要转换为变体标识符的原始名称字符串切片。
#[must_use]
pub fn array_to_tokenstream(array: &[String]) -> TokenStream {
    let variants = array.iter().map(|item| {
        let name = format_ident!("{}", item.to_pascal_case());
        quote! { #name, }
    });

    quote! {
        #(#variants)*
    }
}

/// 将生成的源代码写入 [`OUT_DIR`] 中的文件，若
/// 内容保持不变。
///
/// # Arguments
/// - `new_code` – 要写入的格式化后的源代码字符串。
/// - `out_file` – 要写入的文件名（相对于 [`OUT_DIR`]）。
pub fn write_generated_file(new_code: &str, out_file: &str) {
    let path = Path::new(OUT_DIR).join(out_file);

    if path.exists()
        && let Ok(existing_code) = fs::read_to_string(&path)
        && existing_code == new_code
    {
        return;
    }

    fs::write(&path, new_code).unwrap_or_else(|_| panic!("写入文件失败：{}", path.display()));
}

/// 当 `rustfmt` 不可用或代码格式化失败时返回的错误。
#[derive(Debug)]
pub struct RustFmtError;

/// 通过 `rustfmt` 管道处理来格式化 Rust 源码字符串。
///
/// # Arguments
/// - `unformatted_code` – 要格式化的原始 Rust 源代码。
///
/// # Returns
/// 格式化后的源代码字符串，如果 `rustfmt` 不可用则为 `Err(RustFmtError)`
/// 或格式化失败时。
pub fn format_code(unformatted_code: &str) -> Result<String, RustFmtError> {
    let child_result = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let Ok(mut child) = child_result else {
        return Err(RustFmtError);
    };

    // 把代码写入 rustfmt 的标准输入
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(unformatted_code.as_bytes()).is_err()
    {
        return Err(RustFmtError);
    }

    match child.wait_with_output() {
        Ok(output) if output.status.success() => {
            String::from_utf8(output.stdout).map_err(|_| RustFmtError)
        }
        _ => Err(RustFmtError),
    }
}
