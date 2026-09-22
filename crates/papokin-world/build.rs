#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

const ATTRS: &str = "\
#[allow(clippy::too_many_lines)]
#[allow(clippy::match_same_arms)]
#[allow(clippy::missing_const_for_fn)]
#[allow(clippy::match_single_binding)]
#[must_use]
";

/// 基础数据包（资源），相对于 `CARGO_MANIFEST_DIR`。
///
/// 这些会最先应用。每个数据包必须包含 `data/<namespace>/`。
///
/// 第一个字段是运行时暴露的逻辑数据包（资源）ID。
/// 第二个字段是相对于 `CARGO_MANIFEST_DIR` 的路径。
const DATAPACK_PACKS: &[(&str, &str)] = &[("vanilla", "../../assets/datapacks/26_3")];

/// 容器目录，相对于 `CARGO_MANIFEST_DIR`。
///
/// 每个直接子目录都被视为一个单独的内嵌数据包。
/// 容器数据包按排序顺序应用在 [`DATAPACK_PACKS`] 之后，因此
/// 后加载的数据包会覆盖具有相同资源 id 的早期资源。
const DATAPACK_CONTAINERS: &[&str] = &["../../assets/tests/datapacks"];

/// 原版的隐式命名空间：裸的 `foo` 就等同于 `minecraft:foo`。
const DEFAULT_NAMESPACE: &str = "minecraft";

/// 一个注册表的匹配分支，以完全限定的资源 id 为键。
///
/// 插入会覆盖，因此后加载的数据包会替换先加载数据包的
/// 相同 id 的资源，与原版数据包的覆盖语义一致。
type Registry = BTreeMap<String, String>;

#[derive(Debug)]
struct EmbeddedPack {
    /// 暴露给运行时的逻辑数据包（资源）ID。
    id: String,

    /// 编译时使用的物理源目录。
    path: PathBuf,
}

#[derive(Default)]
struct Arms {
    templates: Registry,
    pools: Registry,
    template_pool_json: Registry,
    processor_list_json: Registry,
    test_instance_json: Registry,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir);

    // 加载顺序：
    //
    // 1. 基础包。
    // 2. 容器包，按目录名排序。
    //
    // 后插入的资源替换先插入的资源。
    let mut packs = Vec::<EmbeddedPack>::new();

    for &(id, relative_path) in DATAPACK_PACKS {
        println!("cargo:rerun-if-changed={relative_path}");

        packs.push(EmbeddedPack {
            id: id.to_string(),
            path: manifest_dir.join(relative_path),
        });
    }

    for container in DATAPACK_CONTAINERS {
        println!("cargo:rerun-if-changed={container}");

        let container_dir = manifest_dir.join(container);

        if !container_dir.is_dir() {
            println!(
                "cargo:warning=missing datapack container: {}",
                container_dir.display()
            );
            continue;
        }

        let mut found = fs::read_dir(&container_dir)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| entry.path().is_dir())
            .collect::<Vec<_>>();

        // 确定性的数据包加载顺序。
        found.sort_by_key(std::fs::DirEntry::file_name);

        for entry in found {
            let id = entry.file_name().into_string().unwrap();
            let path = entry.path();

            packs.push(EmbeddedPack { id, path });
        }
    }

    let mut arms = Arms::default();
    let mut embedded_pack_names = Vec::<String>::new();

    for pack in &packs {
        if embed_pack(&pack.path, &mut arms) {
            // 若配置了多个包，技术上可能出现重复的包 id
            // 位置已包含同名资源包。资源仍使用
            // 正常的加载顺序语义，但公开的资源包列表只应
            // 只包含一次该 id。
            if !embedded_pack_names.iter().any(|id| id == &pack.id) {
                embedded_pack_names.push(pack.id.clone());
            }
        }
    }

    let mut template_out = String::new();

    // ---------------------------------------------------------------------
    // 结构/模板资源查找函数
    // ---------------------------------------------------------------------

    template_out.push_str(&wrap_fn(
        "get_template_bytes",
        "path",
        "Option<&'static [u8]>",
        &arms.templates,
    ));

    template_out.push_str(&wrap_fn(
        "get_pool_elements",
        "pool_id",
        "Option<&'static [&'static str]>",
        &arms.pools,
    ));

    template_out.push_str(&wrap_fn(
        "get_template_pool_json",
        "path",
        "Option<&'static str>",
        &arms.template_pool_json,
    ));

    template_out.push_str(&wrap_fn(
        "get_processor_list_json",
        "path",
        "Option<&'static str>",
        &arms.processor_list_json,
    ));

    // ---------------------------------------------------------------------
    // 结构/模板资源名称列表
    // ---------------------------------------------------------------------

    template_out.push_str(&name_list_fn(
        "_generated_all_template_names",
        &arms.templates,
    ));

    template_out.push_str(&name_list_fn("_generated_all_pool_names", &arms.pools));

    // ---------------------------------------------------------------------
    // 数据包标识
    // ---------------------------------------------------------------------

    template_out.push_str(&string_list_fn(
        "_generated_all_embedded_datapack_names",
        &embedded_pack_names,
    ));

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    fs::write(out_dir.join("template_embeddings.rs"), template_out).unwrap();

    // 测试实例本身就是一类独立的数据包注册表。保持其生成的
    // 资源表与结构模板缓存分开，这样使用者就
    // 无需经由 `generation::structure::template` 访问。
    let mut test_instance_out = String::new();
    test_instance_out.push_str(&wrap_fn(
        "get_test_instance_json",
        "path",
        "Option<&'static str>",
        &arms.test_instance_json,
    ));
    test_instance_out.push_str(&name_list_fn(
        "_generated_all_test_instance_names",
        &arms.test_instance_json,
    ));

    fs::write(
        out_dir.join("test_instance_embeddings.rs"),
        test_instance_out,
    )
    .unwrap();
}

/// 扫描 `<pack>/data/` 下的每个命名空间。
///
///当该目录表示有效的数据包数据根目录时，返回 `true`。
/// 一个有效但为空的 `data/` 目录仍会被视为内嵌
/// 数据包，以便通过 `/datapack list` 展示。
fn embed_pack(pack_dir: &Path, arms: &mut Arms) -> bool {
    let data_dir = pack_dir.join("data");

    if !data_dir.is_dir() {
        println!(
            "cargo:warning=no data/ directory in pack: {}",
            pack_dir.display()
        );
        return false;
    }

    let mut namespaces = fs::read_dir(&data_dir)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| (entry.file_name().into_string().unwrap(), entry.path()))
        .collect::<Vec<_>>();

    namespaces.sort_by(|left, right| left.0.cmp(&right.0));

    for (namespace, namespace_dir) in namespaces {
        embed_namespace(&namespace, &namespace_dir, arms);
    }

    true
}

/// 在单个载体中为所有受支持的注册表生成内嵌资源
/// `data/<namespace>/` 目录。
fn embed_namespace(namespace: &str, namespace_dir: &Path, arms: &mut Arms) {
    let structures_dir = namespace_dir.join("structure");

    if structures_dir.is_dir() {
        let mut pools: BTreeMap<String, Vec<String>> = BTreeMap::new();

        process_structure_dir(
            &structures_dir,
            "",
            &mut arms.templates,
            &mut pools,
            namespace,
        );

        for (pool_id, elements) in pools {
            let mut arm = format!("{} => Some(&[\n", patterns(namespace, &pool_id));

            for element in elements {
                let _ = writeln!(arm, "            \"{element}\",");
            }

            arm.push_str("        ]),");

            arms.pools.insert(qualify(namespace, &pool_id), arm);
        }
    }

    let worldgen_dir = namespace_dir.join("worldgen");

    process_json_dir(
        &worldgen_dir.join("template_pool"),
        "",
        &mut arms.template_pool_json,
        namespace,
    );

    process_json_dir(
        &worldgen_dir.join("processor_list"),
        "",
        &mut arms.processor_list_json,
        namespace,
    );

    process_json_dir(
        &namespace_dir.join("test_instance"),
        "",
        &mut arms.test_instance_json,
        namespace,
    );
}

/// 为每个 `.nbt` 结构递归生成 `include_bytes!` 的 match 分支。
///
/// 它还会将每个目录的直接 NBT 子项分组到现有的
/// Pumpkin 其他地方会用到的结构池辅助工具。
fn process_structure_dir(
    dir: &Path,
    prefix: &str,
    registry: &mut Registry,
    pools: &mut BTreeMap<String, Vec<String>>,
    namespace: &str,
) {
    let mut entries = fs::read_dir(dir)
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();

    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().into_string().unwrap();

        if path.is_dir() {
            let new_prefix = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };

            process_structure_dir(&path, &new_prefix, registry, pools, namespace);

            continue;
        }

        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("nbt"))
        {
            continue;
        }

        let stem = path.file_stem().unwrap().to_string_lossy();

        let template_name = if prefix.is_empty() {
            stem.to_string()
        } else {
            format!("{prefix}/{stem}")
        };

        let resource_id = qualify(namespace, &template_name);
        let absolute_path = path.canonicalize().unwrap();

        registry.insert(
            resource_id.clone(),
            format!(
                "{patterns} => Some(include_bytes!(r#\"{path}\"#)),",
                patterns = patterns(namespace, &template_name),
                path = absolute_path.display(),
            ),
        );

        if !prefix.is_empty() {
            pools
                .entry(prefix.to_string())
                .or_default()
                .push(resource_id);
        }
    }
}

/// 为每个 `.json` 资源递归生成 `include_str!` 的 match 分支。
fn process_json_dir(dir: &Path, prefix: &str, registry: &mut Registry, namespace: &str) {
    if !dir.is_dir() {
        return;
    }

    let mut entries = fs::read_dir(dir)
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();

    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().into_string().unwrap();

        if path.is_dir() {
            let new_prefix = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };

            process_json_dir(&path, &new_prefix, registry, namespace);

            continue;
        }

        let Some(stem) = name.strip_suffix(".json") else {
            continue;
        };

        let id = if prefix.is_empty() {
            stem.to_string()
        } else {
            format!("{prefix}/{stem}")
        };

        let absolute_path = path.canonicalize().unwrap();

        registry.insert(
            qualify(namespace, &id),
            format!(
                "{patterns} => Some(include_str!(r#\"{path}\"#)),",
                patterns = patterns(namespace, &id),
                path = absolute_path.display(),
            ),
        );
    }
}

///返回一个完全限定的 Minecraft 资源 ID。
fn qualify(namespace: &str, id: &str) -> String {
    format!("{namespace}:{id}")
}

/// 为嵌入式资源生成匹配模式。
///
/// 默认命名空间中的资源还接受不带命名空间的裸 id，
/// 因为原版将 `foo` 与 `minecraft:foo` 视为等同。
fn patterns(namespace: &str, id: &str) -> String {
    if namespace == DEFAULT_NAMESPACE {
        format!("        \"{namespace}:{id}\" | \"{id}\"")
    } else {
        format!("        \"{namespace}:{id}\"")
    }
}

/// 生成一个资源查找函数。
fn wrap_fn(name: &str, argument: &str, return_type: &str, registry: &Registry) -> String {
    let mut arms = String::new();

    for arm in registry.values() {
        let _ = writeln!(arms, "{arm}");
    }

    format!(
        "{ATTRS}\
         pub fn {name}({argument}: &str) -> {return_type} {{\n\
         \x20   match {argument} {{\n\
         {arms}\
         \x20       _ => None,\n\
         \x20   }}\n\
         }}\n\n"
    )
}

/// 生成包含注册表中所有资源 ID 的列表。
fn name_list_fn(name: &str, registry: &Registry) -> String {
    let mut output = format!(
        "#[must_use]\n\
         #[allow(clippy::too_many_lines, clippy::large_stack_arrays)]\n\
         pub const fn {name}() -> &'static [&'static str] {{\n\
         \x20   &[\n"
    );

    for id in registry.keys() {
        let _ = writeln!(output, "        \"{id}\",");
    }

    output.push_str("    ]\n}\n\n");

    output
}

/// 生成一个字符串静态列表。
///
/// 与 [`name_list_fn`] 不同，此方法用于非资源 id 的值，
/// 例如内嵌数据包的逻辑名称等。
fn string_list_fn(name: &str, values: &[String]) -> String {
    let mut output = format!(
        "#[must_use]\n\
         #[allow(clippy::too_many_lines, clippy::large_stack_arrays)]\n\
         pub const fn {name}() -> &'static [&'static str] {{\n\
         \x20   &[\n"
    );

    for value in values {
        // 调试格式化会生成正确转义的 Rust 字符串字面量。
        let _ = writeln!(output, "        {value:?},");
    }

    output.push_str("    ]\n}\n\n");

    output
}
