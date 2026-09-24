pub mod attribute;
pub mod biome;
pub mod damage_type;
pub mod data_component;
pub mod enchantment;
pub mod entity_status;
pub mod entity_type;
pub mod game_event;
pub mod game_rules;
pub mod java_packet;
pub mod packet_mapping;
pub mod particle;
pub mod potion;
pub mod screen;
pub mod sound;
pub mod statistic;
pub mod utils;

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

pub const WIT_OUT_DIR: &str = "../../crates/papokin-plugin-wit/v0.1";
pub const MAPPING_OUT_DIR: &str = "../../crates/papokin/src/plugin/loader/wasm/wasm_host/wit/v0_1";

pub fn main() {
    fs::create_dir_all(WIT_OUT_DIR).expect("创建 WIT 输出目录失败");

    type BuildFn = fn() -> String;
    let build_functions: Vec<(BuildFn, &str)> = vec![
        (particle::build, "particles.wit"),
        (sound::build, "sounds.wit"),
        (entity_type::build, "entity-types.wit"),
        (java_packet::build, "java-packets.wit"),
        (data_component::build, "data-components.wit"),
        (enchantment::build, "enchantments.wit"),
        (biome::build, "biomes.wit"),
        (attribute::build, "attributes.wit"),
        (damage_type::build, "damage-types.wit"),
        (screen::build, "screens.wit"),
        (statistic::build, "statistics.wit"),
        (game_rules::build, "game-rules.wit"),
        (game_event::build, "game-events.wit"),
        (potion::build, "potions.wit"),
        (entity_status::build, "entity-statuses.wit"),
    ];

    for (build_fn, file) in build_functions {
        println!("正在为 {} 生成 WIT", file);
        let wit_code = build_fn();
        write_generated_wit(&wit_code, file);
    }

    println!("正在生成 Java 数据包映射");
    let mut mapping = packet_mapping::build_java_mapping();

    mapping = format_code(&mapping).unwrap_or(mapping);

    let mapping_path = Path::new(MAPPING_OUT_DIR).join("generated_packets.rs");
    fs::write(&mapping_path, mapping).expect("写入数据包映射失败");
}

fn write_generated_wit(new_code: &str, out_file: &str) {
    let path = Path::new(WIT_OUT_DIR).join(out_file);

    if path.exists()
        && let Ok(existing_code) = fs::read_to_string(&path)
        && existing_code == new_code
    {
        return;
    }

    fs::write(&path, new_code).unwrap_or_else(|_| panic!("写入文件失败：{}", path.display()));
}

/// 当 `rustfmt` 不可用或代码格式化失败时返回的错误。
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
