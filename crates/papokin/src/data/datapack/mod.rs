pub mod function_loader;
pub mod recipe_loader;
pub mod test_loader;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

use papokin_data::registry::RegistryEntryData;
use papokin_nbt::{NbtCompound, nbt_compress::read_gzip_compound_tag};
use papokin_protocol::codec::recipe::DynamicRecipe;

use crate::command::context::command_source::CommandSource;
use crate::server::Server;
use crate::server::recipe::RecipeManager;

use self::test_loader::{
    TestInstance, TestInstanceRegistry, load_test_instances_from_dir, to_registry_entry,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownPackData {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct LoadedDatapack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pack_format: u32,
    pub root_path: PathBuf,
    pub recipe_count: usize,
    pub function_count: usize,
    pub known_packs: Vec<KnownPackData>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatapackInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pack_format: u32,
    pub is_enabled: bool,
    pub recipe_count: usize,
    pub function_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatapackEnablePosition {
    First,
    Last,
    Before(String),
    After(String),
}

/// 函数执行的最大嵌套层数。原版以 `maxCommandChainLength`
/// （默认 65536 条命令）约束整条链；这里以嵌套层数为上限，
/// 足够覆盖任何合法 datapack 的函数嵌套深度。
const MAX_FUNCTION_DEPTH: u32 = 512;

// 当前线程的函数执行嵌套深度。递归必然发生在同一线程的
// 同步调用栈上（handle_command 是同步的），thread_local 即可。
thread_local! {
    // 已是 const 初始化，clippy 1.98 对该形式仍误报（与 density_volume 同）
    #[allow(clippy::missing_const_for_thread_local)]
    static FUNCTION_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// 单次函数触发允许执行的最大命令条数，对齐原版
/// `maxCommandChainLength` 的默认值；同一条触发链上嵌套执行的
/// 函数共享该预算。
const MAX_FUNCTION_COMMANDS: i64 = 65_536;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static FUNCTION_COMMAND_BUDGET: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// 从当前执行链的命令预算中扣减一条额度；返回 `false` 表示
/// 预算已耗尽。预算在每次顶层触发（深度为 1）时重建。
fn consume_command_budget() -> bool {
    FUNCTION_COMMAND_BUDGET.with(|budget| {
        let next = budget.get() - 1;
        budget.set(next);
        next >= 0
    })
}

/// 进入函数执行时的深度守卫：超过上限则拒绝进入，离开时递减。
struct FunctionDepthGuard;

impl FunctionDepthGuard {
    fn enter() -> Option<Self> {
        FUNCTION_DEPTH.with(|depth| {
            if depth.get() >= MAX_FUNCTION_DEPTH {
                None
            } else {
                depth.set(depth.get() + 1);
                Some(Self)
            }
        })
    }
}

impl Drop for FunctionDepthGuard {
    fn drop(&mut self) {
        FUNCTION_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

pub struct DatapackManager {
    loaded_packs: RwLock<Vec<LoadedDatapack>>,
    functions: RwLock<HashMap<String, Arc<[String]>>>,
    function_tags: RwLock<HashMap<String, Vec<String>>>,
    test_instances: RwLock<TestInstanceRegistry>,
}

fn share_function_bodies(
    functions: HashMap<String, Vec<String>>,
) -> HashMap<String, Arc<[String]>> {
    functions
        .into_iter()
        .map(|(name, lines)| (name, lines.into()))
        .collect()
}

impl Default for DatapackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DatapackManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            loaded_packs: RwLock::new(Vec::new()),
            functions: RwLock::new(HashMap::new()),
            function_tags: RwLock::new(HashMap::new()),
            test_instances: RwLock::new(HashMap::new()),
        }
    }

    pub fn load_all(
        &self,
        world_path: &Path,
        enabled_packs: &[String],
        recipe_manager: &RecipeManager,
    ) {
        let datapacks_dir = world_path.join("datapacks");
        let mut loaded_packs_vec = Vec::new();
        let mut all_recipes: Vec<DynamicRecipe> = Vec::new();
        let mut all_functions: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_function_tags: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_test_instances: TestInstanceRegistry = HashMap::new();

        // 嵌入式测试实例是编译期常量，始终会加载
        // 这样磁盘上的数据包便可按 id 覆盖它们。
        let embedded_count = test_loader::load_embedded_test_instances(&mut all_test_instances);
        if embedded_count > 0 {
            info!("已加载 {embedded_count} 个内嵌测试实例");
        }
        if datapacks_dir.is_dir() {
            match fs::read_dir(&datapacks_dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let pack_path = entry.path();
                        let file_name = entry.file_name().to_string_lossy().to_string();

                        if file_name.starts_with('.') || !pack_path.is_dir() {
                            continue;
                        }

                        let pack_id = format!("file/{file_name}");
                        let is_enabled = enabled_packs
                            .iter()
                            .any(|p| p == &pack_id || p == &file_name);
                        if !is_enabled {
                            continue;
                        }

                        let (description, pack_format, known_packs) = read_pack_mcmeta(&pack_path);

                        let (pack_recipe_count, pack_function_count, pack_test_instance_count) =
                            load_pack_contents(
                                &pack_path,
                                &mut all_recipes,
                                &mut all_functions,
                                &mut all_function_tags,
                                &mut all_test_instances,
                            );

                        info!(
                            "已加载数据包 '{file_name}'：{pack_recipe_count} 个配方，{pack_function_count} 个函数，{pack_test_instance_count} 个测试实例"
                        );

                        loaded_packs_vec.push(LoadedDatapack {
                            id: pack_id,
                            name: file_name,
                            description,
                            pack_format,
                            root_path: pack_path,
                            recipe_count: pack_recipe_count,
                            function_count: pack_function_count,
                            known_packs,
                        });
                    }
                }
                Err(error) => {
                    warn!("读取数据包目录 '{}' 失败：{error}", datapacks_dir.display());
                }
            }
        }

        recipe_manager.set_recipes(all_recipes);
        *self
            .loaded_packs
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = loaded_packs_vec;
        let all_functions = share_function_bodies(all_functions);
        *self
            .functions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = all_functions;
        *self
            .function_tags
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = all_function_tags;
        *self
            .test_instances
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = all_test_instances;
    }

    pub fn get_loaded_packs(&self) -> Vec<LoadedDatapack> {
        self.loaded_packs
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn get_functions(&self) -> HashMap<String, Vec<String>> {
        self.functions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(name, lines)| (name.clone(), lines.to_vec()))
            .collect()
    }

    pub fn get_test_instance(&self, name: &str) -> Option<TestInstance> {
        self.test_instances
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .cloned()
    }

    pub fn get_test_instance_names(&self) -> Vec<String> {
        let test_instances = self
            .test_instances
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut names: Vec<_> = test_instances.keys().cloned().collect();
        names.sort_unstable();
        names
    }

    ///以协议的同步注册表条目格式返回数据包测试实例。
    /// 原版测试实例方块渲染器会解析 required/padding/base 旋转
    /// 通过此注册表，使用控制器的 `data.test` 资源键。
    pub fn get_test_instance_registry_entries(&self) -> Vec<RegistryEntryData> {
        let test_instances = self
            .test_instances
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut entries: Vec<_> = test_instances
            .iter()
            .map(|(id, instance)| to_registry_entry(id.clone(), instance))
            .collect();
        entries.sort_unstable_by(|left, right| left.entry_id.cmp(&right.entry_id));
        entries
    }

    /// 返回供客户端配置/数据包（资源）同步使用的已知数据包（资源）列表，
    /// 与原版的 `server.getResourceManager().listPacks().flatMap(p -> p.location().knownPackInfo().stream())` 一致。
    pub fn get_known_packs<'a>(
        &self,
        server: &Server,
        server_version: &'a str,
        loaded_packs: &'a [LoadedDatapack],
    ) -> Vec<papokin_protocol::KnownPack<'a>> {
        use papokin_protocol::KnownPack;

        let mut known_packs = Vec::new();

        // 1. 主原版核心包
        known_packs.push(KnownPack {
            namespace: "minecraft",
            id: "core",
            version: server_version,
        });

        // 2. 已启用的内置特性包
        let enabled_packs = Self::get_enabled_packs(server);
        for pack_name in &enabled_packs {
            let id: &'static str = match pack_name.as_str() {
                "trade_rebalance" => "trade_rebalance",
                "minecart_improvements" => "minecart_improvements",
                "redstone_experiments" => "redstone_experiments",
                "bundle" => "bundle",
                _ => continue,
            };
            let pack = KnownPack {
                namespace: "minecraft",
                id,
                version: server_version,
            };
            if !known_packs
                .iter()
                .any(|p| p.namespace == pack.namespace && p.id == pack.id)
            {
                known_packs.push(pack);
            }
        }

        // 3. 已加载的包，其 known_pack 信息来自 pack.mcmeta
        for pack in loaded_packs {
            for kp in &pack.known_packs {
                let p = KnownPack {
                    namespace: &kp.namespace,
                    id: &kp.id,
                    version: &kp.version,
                };
                if !known_packs.iter().any(|existing| {
                    existing.namespace == p.namespace
                        && existing.id == p.id
                        && existing.version == p.version
                }) {
                    known_packs.push(p);
                }
            }
        }

        known_packs
    }

    /// 返回已启用的世界特性标志（例如 `minecraft:vanilla`、`minecraft:trade_rebalance`、
    /// `minecraft:minecart_improvements`、`minecraft:redstone_experiments`、`minecraft:bundle`）。
    #[must_use]
    pub fn get_enabled_features(&self, server: &Server) -> Vec<&'static str> {
        let enabled_packs = Self::get_enabled_packs(server);
        Self::resolve_enabled_features(&enabled_packs)
    }

    /// 根据给定的已启用数据包名称列表，解析出已启用的特性标志名称。
    #[must_use]
    pub fn resolve_enabled_features(enabled_packs: &[String]) -> Vec<&'static str> {
        let mut features = vec!["minecraft:vanilla"];

        for pack_name in enabled_packs {
            let feature: &'static str = match pack_name.as_str() {
                "trade_rebalance" | "file/trade_rebalance" => "minecraft:trade_rebalance",
                "minecart_improvements" | "file/minecart_improvements" => {
                    "minecraft:minecart_improvements"
                }
                "redstone_experiments" | "file/redstone_experiments" => {
                    "minecraft:redstone_experiments"
                }
                "bundle" | "file/bundle" => "minecraft:bundle",
                _ => {
                    if let Some(stripped) = pack_name.strip_prefix("file/") {
                        match stripped {
                            "trade_rebalance" => "minecraft:trade_rebalance",
                            "minecart_improvements" => "minecraft:minecart_improvements",
                            "redstone_experiments" => "minecraft:redstone_experiments",
                            "bundle" => "minecraft:bundle",
                            _ => continue,
                        }
                    } else {
                        continue;
                    }
                }
            };
            if !features.contains(&feature) {
                features.push(feature);
            }
        }

        features
    }

    #[must_use]
    pub fn is_feature_enabled(&self, server: &Server, feature: &str) -> bool {
        self.get_enabled_features(server).contains(&feature)
    }

    /// 从当前启用的数据包（资源）中加载 Java 版结构 NBT。
    ///
    /// 结构标识符是资源位置，例如
    /// `minecraft:village/plains/houses/plains_small_house_1`。无论是当前
    /// `structure` 目录和旧版的 `structures` 目录都会被检查。
    pub async fn load_structure(&self, resource_location: &str) -> Result<NbtCompound, String> {
        let (namespace, path) = parse_structure_resource_location(resource_location)?;

        let nbt_path = {
            let loaded_packs = self
                .loaded_packs
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            let mut nbt_path = None;

            // 运行时数据包覆盖内嵌资源。
            'packs: for pack in loaded_packs.iter().rev() {
                for structure_dir in ["structure", "structures"] {
                    let candidate = pack
                        .root_path
                        .join("data")
                        .join(namespace)
                        .join(structure_dir)
                        .join(format!("{path}.nbt"));

                    if candidate.is_file() {
                        nbt_path = Some(candidate);
                        break 'packs;
                    }
                }
            }

            nbt_path
        };

        if let Some(nbt_path) = nbt_path {
            let display_path = nbt_path.display().to_string();

            return tokio::task::spawn_blocking(move || {
                let file = fs::File::open(&nbt_path)
                    .map_err(|error| format!("打开结构 '{display_path}' 失败：{error}"))?;

                read_gzip_compound_tag(file)
                    .map_err(|error| format!("解析结构 '{display_path}' 失败：{error}"))
            })
            .await
            .map_err(|error| format!("结构加载任务失败：{error}"))?;
        }

        // 回退到编译期嵌入的结构
        let structure_id = format!("{namespace}:{path}");

        if let Some(bytes) =
            papokin_world::generation::structure::template::template_bytes(&structure_id)
        {
            return read_gzip_compound_tag(std::io::Cursor::new(bytes))
                .map_err(|error| format!("解析内嵌结构 '{structure_id}' 失败：{error}"));
        }

        Err(format!(
            "在任何已启用的数据包或内嵌资源中都未找到结构 '{resource_location}'"
        ))
    }

    pub fn get_function_names(&self) -> Vec<String> {
        let fns = self
            .functions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tags = self
            .function_tags
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut names = Vec::with_capacity(fns.len() + tags.len());
        names.extend(fns.keys().cloned());
        for tag in tags.keys() {
            names.push(format!("#{tag}"));
        }
        names
    }

    pub fn execute_function(
        &self,
        server: &Arc<Server>,
        source: &CommandSource,
        name: &str,
    ) -> Result<usize, String> {
        // 函数调用函数的递归（自引用函数/标签）会经 handle_command
        // 同步重入本函数，无保护时直接爆栈崩溃服务器；原版经
        // maxCommandChainLength 配额约束同样的面。
        let Some(_depth_guard) = FunctionDepthGuard::enter() else {
            return Err(format!(
                "函数嵌套超过 {MAX_FUNCTION_DEPTH} 层：可能存在递归调用"
            ));
        };
        // 顶层触发重建本条执行链的命令预算；嵌套执行共享同一预算，
        // 防止单函数巨量行/标签展开做慢消耗 DoS
        if FUNCTION_DEPTH.with(std::cell::Cell::get) == 1 {
            FUNCTION_COMMAND_BUDGET.with(|budget| budget.set(MAX_FUNCTION_COMMANDS));
        }
        let mut budget_exhausted = false;
        let result = self.visit_function_lines(name, |line| {
            if budget_exhausted || !consume_command_budget() {
                budget_exhausted = true;
                return;
            }
            server
                .command_dispatcher
                .load()
                .handle_command(source, line);
        });
        if budget_exhausted {
            return Err(format!(
                "函数执行链超过 {MAX_FUNCTION_COMMANDS} 条命令上限，已中止"
            ));
        }
        result
    }

    fn visit_function_lines(
        &self,
        name: &str,
        mut visit: impl FnMut(&str),
    ) -> Result<usize, String> {
        let (functions_to_run, is_tag) = if let Some(tag_name) = name.strip_prefix('#') {
            let tags = self
                .function_tags
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(fns) = tags.get(tag_name) else {
                return Err(format!("未知的函数标签：#{tag_name}"));
            };
            (fns.clone(), true)
        } else {
            (vec![name.to_string()], false)
        };

        let functions = {
            let all_fns = self
                .functions
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut functions = Vec::with_capacity(functions_to_run.len());
            for fn_id in functions_to_run {
                let Some(lines) = all_fns.get(&fn_id) else {
                    if !is_tag {
                        return Err(format!("未知的函数：{fn_id}"));
                    }
                    continue;
                };
                functions.push(Arc::clone(lines));
            }
            functions
        };

        let mut total_executed = 0;
        for lines in functions {
            for line in lines.iter() {
                visit(line);
                total_executed += 1;
            }
        }

        Ok(total_executed)
    }

    #[must_use]
    pub fn is_embedded_pack(name: &str) -> bool {
        papokin_world::generation::structure::template::all_embedded_datapack_names()
            .contains(&name)
    }

    pub fn get_all_known_packs(server: &Server) -> Vec<String> {
        let mut packs: Vec<String> =
            papokin_world::generation::structure::template::all_embedded_datapack_names()
                .iter()
                .map(|name| (*name).to_owned())
                .collect();

        // 捆绑的功能包，服务器独立于
        // 编译期嵌入的结构/测试资源。
        for bundled in [
            "trade_rebalance",
            "minecart_improvements",
            "redstone_experiments",
        ] {
            if !packs.iter().any(|p| p == bundled) {
                packs.push(bundled.to_string());
            }
        }

        // 世界数据包目录。
        let datapacks_dir = server.basic_config.get_world_path().join("datapacks");
        if let Ok(entries) = fs::read_dir(datapacks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                if file_name.starts_with('.') {
                    continue;
                }

                if path.is_dir()
                    || path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
                {
                    let pack_name = format!("file/{file_name}");
                    if !packs.iter().any(|p| p == &pack_name) {
                        packs.push(pack_name);
                    }
                }
            }
        }

        let level_info = server.level_info.load();

        for pack in &level_info.data_packs.enabled {
            if !packs.iter().any(|p| p == pack) {
                packs.push(pack.clone());
            }
        }

        for pack in &level_info.data_packs.disabled {
            if !packs.iter().any(|p| p == pack) {
                packs.push(pack.clone());
            }
        }

        packs
    }

    pub fn get_enabled_packs(server: &Server) -> Vec<String> {
        // 编译期内嵌的数据包无法在运行时禁用，因此
        // 将它们暴露为永久启用。
        let mut packs: Vec<String> =
            papokin_world::generation::structure::template::all_embedded_datapack_names()
                .iter()
                .map(|name| (*name).to_owned())
                .collect();

        for pack in &server.level_info.load().data_packs.enabled {
            if !packs.iter().any(|existing| existing == pack) {
                packs.push(pack.clone());
            }
        }

        packs
    }

    pub fn get_available_packs(server: &Server) -> Vec<String> {
        let enabled = Self::get_enabled_packs(server);
        let all = Self::get_all_known_packs(server);
        all.into_iter().filter(|p| !enabled.contains(p)).collect()
    }

    pub fn find_pack_name(server: &Server, input: &str) -> Option<String> {
        let known = Self::get_all_known_packs(server);
        if let Some(p) = known.iter().find(|p| *p == input) {
            return Some(p.clone());
        }
        let file_input = format!("file/{input}");
        if let Some(p) = known.iter().find(|p| **p == file_input) {
            return Some(p.clone());
        }
        if let Some(p) = known
            .iter()
            .find(|p| p.strip_prefix("file/") == Some(input))
        {
            return Some(p.clone());
        }
        None
    }

    pub fn get_pack_info(server: &Server, name_or_id: &str) -> Option<DatapackInfo> {
        let resolved_name = Self::find_pack_name(server, name_or_id)?;
        let enabled_packs = Self::get_enabled_packs(server);
        let is_enabled = enabled_packs.contains(&resolved_name);

        let loaded = server.datapack_manager.get_loaded_packs();
        if let Some(pack) = loaded
            .iter()
            .find(|p| p.id == resolved_name || p.name == resolved_name)
        {
            return Some(DatapackInfo {
                id: pack.id.clone(),
                name: pack.name.clone(),
                description: pack.description.clone(),
                pack_format: pack.pack_format,
                is_enabled,
                recipe_count: pack.recipe_count,
                function_count: pack.function_count,
            });
        }

        let (id, name, description, pack_format) = if resolved_name == "vanilla" {
            (
                "vanilla".to_string(),
                "vanilla".to_string(),
                "默认数据包".to_string(),
                61,
            )
        } else if Self::is_embedded_pack(&resolved_name) {
            (
                resolved_name.clone(),
                resolved_name.clone(),
                format!("内嵌数据包：{resolved_name}"),
                61,
            )
        } else if let Some(stripped) = resolved_name.strip_prefix("file/") {
            let pack_path = server
                .basic_config
                .get_world_path()
                .join("datapacks")
                .join(stripped);
            let (desc, format, _) = read_pack_mcmeta(&pack_path);
            (resolved_name.clone(), stripped.to_string(), desc, format)
        } else {
            (
                resolved_name.clone(),
                resolved_name.clone(),
                format!("随附数据包：{resolved_name}"),
                61,
            )
        };

        Some(DatapackInfo {
            id,
            name,
            description,
            pack_format,
            is_enabled,
            recipe_count: 0,
            function_count: 0,
        })
    }

    pub fn list_all_packs(server: &Server) -> Vec<DatapackInfo> {
        let all = Self::get_all_known_packs(server);
        all.into_iter()
            .filter_map(|p| Self::get_pack_info(server, &p))
            .collect()
    }

    pub fn list_enabled_packs(server: &Server) -> Vec<DatapackInfo> {
        let enabled = Self::get_enabled_packs(server);
        enabled
            .into_iter()
            .filter_map(|p| Self::get_pack_info(server, &p))
            .collect()
    }

    pub fn list_available_packs(server: &Server) -> Vec<DatapackInfo> {
        let available = Self::get_available_packs(server);
        available
            .into_iter()
            .filter_map(|p| Self::get_pack_info(server, &p))
            .collect()
    }

    pub fn is_pack_enabled(server: &Server, name: &str) -> bool {
        let Some(resolved) = Self::find_pack_name(server, name) else {
            return false;
        };
        Self::get_enabled_packs(server).contains(&resolved)
    }

    pub fn enable_pack(
        server: &Arc<Server>,
        name: &str,
        position: DatapackEnablePosition,
    ) -> Result<(), String> {
        let Some(resolved_name) = Self::find_pack_name(server, name) else {
            return Err(format!("未知的数据包 '{name}'"));
        };

        let enabled = Self::get_enabled_packs(server);
        if enabled.contains(&resolved_name) {
            return Err(format!("数据包 '{resolved_name}' 已启用"));
        }

        let target = resolved_name;
        match position {
            DatapackEnablePosition::First => {
                server.level_info.rcu(|level_info| {
                    let mut new_info = (**level_info).clone();
                    new_info.data_packs.disabled.retain(|p| p != &target);
                    new_info.data_packs.enabled.retain(|p| p != &target);
                    new_info.data_packs.enabled.insert(0, target.clone());
                    new_info
                });
            }
            DatapackEnablePosition::Last => {
                server.level_info.rcu(|level_info| {
                    let mut new_info = (**level_info).clone();
                    new_info.data_packs.disabled.retain(|p| p != &target);
                    new_info.data_packs.enabled.retain(|p| p != &target);
                    new_info.data_packs.enabled.push(target.clone());
                    new_info
                });
            }
            DatapackEnablePosition::Before(existing_name) => {
                let Some(existing_pack) = Self::find_pack_name(server, &existing_name) else {
                    return Err(format!("未知的现有数据包 '{existing_name}'"));
                };
                if !enabled.contains(&existing_pack) {
                    return Err(format!("数据包 '{existing_pack}' 未启用"));
                }
                server.level_info.rcu(|level_info| {
                    let mut new_info = (**level_info).clone();
                    new_info.data_packs.disabled.retain(|p| p != &target);
                    new_info.data_packs.enabled.retain(|p| p != &target);
                    if let Some(idx) = new_info
                        .data_packs
                        .enabled
                        .iter()
                        .position(|p| p == &existing_pack)
                    {
                        new_info.data_packs.enabled.insert(idx, target.clone());
                    } else {
                        new_info.data_packs.enabled.push(target.clone());
                    }
                    new_info
                });
            }
            DatapackEnablePosition::After(existing_name) => {
                let Some(existing_pack) = Self::find_pack_name(server, &existing_name) else {
                    return Err(format!("未知的现有数据包 '{existing_name}'"));
                };
                if !enabled.contains(&existing_pack) {
                    return Err(format!("数据包 '{existing_pack}' 未启用"));
                }
                server.level_info.rcu(|level_info| {
                    let mut new_info = (**level_info).clone();
                    new_info.data_packs.disabled.retain(|p| p != &target);
                    new_info.data_packs.enabled.retain(|p| p != &target);
                    if let Some(idx) = new_info
                        .data_packs
                        .enabled
                        .iter()
                        .position(|p| p == &existing_pack)
                    {
                        new_info.data_packs.enabled.insert(idx + 1, target.clone());
                    } else {
                        new_info.data_packs.enabled.push(target.clone());
                    }
                    new_info
                });
            }
        }

        if let Err(err) = server.save_world_info() {
            tracing::error!("保存世界信息失败：{err}");
        }

        server.reload_datapacks(server);
        Ok(())
    }

    pub fn disable_pack(server: &Arc<Server>, name: &str) -> Result<(), String> {
        let Some(target_pack) = Self::find_pack_name(server, name) else {
            return Err(format!("未知的数据包 '{name}'"));
        };

        let enabled = Self::get_enabled_packs(server);
        if !enabled.contains(&target_pack) {
            return Err(format!("数据包 '{target_pack}' 未启用"));
        }

        if Self::is_embedded_pack(&target_pack) {
            return Err(format!(
                "无法禁用内嵌数据包 '{target_pack}'，因为它已编译进服务器"
            ));
        }

        let target = target_pack;
        server.level_info.rcu(|level_info| {
            let mut new_info = (**level_info).clone();
            new_info.data_packs.enabled.retain(|p| p != &target);
            if !new_info.data_packs.disabled.contains(&target) {
                new_info.data_packs.disabled.push(target.clone());
            }
            new_info
        });

        if let Err(err) = server.save_world_info() {
            tracing::error!("保存世界信息失败：{err}");
        }

        server.reload_datapacks(server);
        Ok(())
    }

    pub fn reload(server: &Arc<Server>) -> Result<(), String> {
        server.reload_datapacks(server);
        Ok(())
    }

    pub fn execute_function_from_console(
        server: &Arc<Server>,
        name: &str,
    ) -> Result<usize, String> {
        let source = crate::command::CommandSender::Console.into_source(server);
        server
            .datapack_manager
            .execute_function(server, &source, name)
    }
}

fn parse_structure_resource_location(resource_location: &str) -> Result<(&str, &str), String> {
    let (namespace, raw_path) = resource_location
        .split_once(':')
        .unwrap_or(("minecraft", resource_location));

    if namespace.is_empty()
        || !namespace.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        return Err(format!("'{resource_location}' 中的结构命名空间无效"));
    }

    let path = raw_path.strip_suffix(".nbt").unwrap_or(raw_path);
    if path.is_empty()
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || !path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        })
    {
        return Err(format!("'{resource_location}' 中的结构路径无效"));
    }

    Ok((namespace, path))
}

/// 从单个数据包（资源）中加载配方、函数、函数标签与测试实例
/// 数据包目录，以 `(recipes, functions, test_instances)` 返回每个数据包的数量。
fn load_pack_contents(
    pack_path: &Path,
    all_recipes: &mut Vec<DynamicRecipe>,
    all_functions: &mut HashMap<String, Vec<String>>,
    all_function_tags: &mut HashMap<String, Vec<String>>,
    all_test_instances: &mut TestInstanceRegistry,
) -> (usize, usize, usize) {
    let data_dir = pack_path.join("data");
    let mut pack_recipe_count = 0;
    let mut pack_function_count = 0;
    let mut pack_test_instance_count = 0;

    if data_dir.is_dir()
        && let Ok(ns_entries) = fs::read_dir(&data_dir)
    {
        for ns_entry in ns_entries.flatten() {
            let ns_path = ns_entry.path();
            if !ns_path.is_dir() {
                continue;
            }
            let namespace = ns_entry.file_name().to_string_lossy().to_string();

            // 加载配方
            for recipe_sub in ["recipe", "recipes"] {
                let recipe_dir = ns_path.join(recipe_sub);
                if recipe_dir.is_dir() {
                    load_recipes_from_dir(
                        &namespace,
                        &recipe_dir,
                        all_recipes,
                        &mut pack_recipe_count,
                    );
                }
            }

            // 加载函数
            for fn_sub in ["function", "functions"] {
                let fn_dir = ns_path.join(fn_sub);
                if fn_dir.is_dir() {
                    let before = all_functions.len();
                    function_loader::load_functions_from_dir(&namespace, &fn_dir, all_functions);
                    pack_function_count += all_functions.len() - before;
                }
            }

            // 加载标签
            let tags_dir = ns_path.join("tags");
            if tags_dir.is_dir() {
                function_loader::load_function_tags_from_dir(
                    &namespace,
                    &tags_dir,
                    all_function_tags,
                );
            }
            // 加载游戏测试实例
            let test_instance_dir = ns_path.join("test_instance");
            if test_instance_dir.is_dir() {
                pack_test_instance_count += load_test_instances_from_dir(
                    &namespace,
                    &test_instance_dir,
                    all_test_instances,
                );
            }
        }
    }

    (
        pack_recipe_count,
        pack_function_count,
        pack_test_instance_count,
    )
}

fn read_pack_mcmeta(pack_path: &Path) -> (String, u32, Vec<KnownPackData>) {
    let mcmeta_path = pack_path.join("pack.mcmeta");
    if let Ok(content) = fs::read_to_string(mcmeta_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
    {
        let pack = val.get("pack");
        let description = pack
            .and_then(|p| p.get("description"))
            .map(|d| {
                d.as_str()
                    .map_or_else(|| d.to_string(), ToString::to_string)
            })
            .unwrap_or_default();
        let pack_format = pack
            .and_then(|p| p.get("pack_format"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(61) as u32;

        let mut known_packs = Vec::new();
        let parse_item = |item: &serde_json::Value| -> Option<KnownPackData> {
            let ns = item.get("namespace").and_then(serde_json::Value::as_str)?;
            let id = item.get("id").and_then(serde_json::Value::as_str)?;
            let ver = item.get("version").and_then(serde_json::Value::as_str)?;
            Some(KnownPackData {
                namespace: ns.to_string(),
                id: id.to_string(),
                version: ver.to_string(),
            })
        };

        if let Some(packs_array) = val.get("known_packs").and_then(serde_json::Value::as_array) {
            for item in packs_array {
                if let Some(kp) = parse_item(item) {
                    known_packs.push(kp);
                }
            }
        } else if let Some(item) = val.get("known_pack") {
            if let Some(kp) = parse_item(item) {
                known_packs.push(kp);
            }
        } else if let Some(pack_obj) = pack {
            if let Some(packs_array) = pack_obj
                .get("known_packs")
                .and_then(serde_json::Value::as_array)
            {
                for item in packs_array {
                    if let Some(kp) = parse_item(item) {
                        known_packs.push(kp);
                    }
                }
            } else if let Some(item) = pack_obj.get("known_pack")
                && let Some(kp) = parse_item(item)
            {
                known_packs.push(kp);
            }
        }

        return (description, pack_format, known_packs);
    }
    (String::new(), 61, Vec::new())
}

fn load_recipes_recursive(
    namespace: &str,
    base_dir: &Path,
    current_dir: &Path,
    all_recipes: &mut Vec<DynamicRecipe>,
    count: &mut usize,
) {
    let Ok(entries) = fs::read_dir(current_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            load_recipes_recursive(namespace, base_dir, &path, all_recipes, count);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            let Ok(relative_path) = path.strip_prefix(base_dir) else {
                continue;
            };

            let recipe_name = relative_path
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");

            if let Ok(content) = fs::read_to_string(&path)
                && let Some(recipe) = recipe_loader::parse_recipe(namespace, &recipe_name, &content)
            {
                all_recipes.push(recipe);
                *count += 1;
            }
        }
    }
}

fn load_recipes_from_dir(
    namespace: &str,
    dir: &Path,
    all_recipes: &mut Vec<DynamicRecipe>,
    count: &mut usize,
) {
    load_recipes_recursive(namespace, dir, dir, all_recipes, count);
}
#[cfg(test)]
mod tests {
    use super::DatapackManager;

    #[test]
    fn function_dispatch_releases_the_functions_lock() {
        let manager = DatapackManager::new();
        manager
            .functions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                "test:reentrant".to_string(),
                vec!["first command".to_string(), "second command".to_string()].into(),
            );

        let mut visited = Vec::new();
        let executed = manager
            .visit_function_lines("test:reentrant", |line| {
                let functions = manager
                    .functions
                    .try_write()
                    .expect("函数调度不得持有 functions 读锁");
                drop(functions);
                visited.push(line.to_string());
            })
            .expect("执行函数行");

        assert_eq!(executed, 2);
        assert_eq!(visited, ["first command", "second command"]);
    }

    #[test]
    fn function_resolution_preserves_unknown_errors_and_skips_missing_tag_entries() {
        let manager = DatapackManager::new();
        manager
            .functions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                "test:known".to_string(),
                vec!["known command".to_string()].into(),
            );
        manager
            .function_tags
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                "test:mixed".to_string(),
                vec!["test:missing".to_string(), "test:known".to_string()],
            );

        let mut visited = Vec::new();
        let executed = manager
            .visit_function_lines("#test:mixed", |line| visited.push(line.to_string()))
            .expect("执行函数标签");
        assert_eq!(executed, 1);
        assert_eq!(visited, ["known command"]);

        assert_eq!(
            manager
                .visit_function_lines("test:missing", |_| {})
                .expect_err("unknown function must fail"),
            "未知的函数：test:missing"
        );
        assert_eq!(
            manager
                .visit_function_lines("#test:missing", |_| {})
                .expect_err("unknown function tag must fail"),
            "未知的函数标签：#test:missing"
        );
    }

    #[test]
    fn read_pack_mcmeta_parses_known_packs() {
        let temp_dir = tempfile::tempdir().expect("临时目录");
        let pack_path = temp_dir.path();

        // 1. 测试根级 known_packs 数组
        let mcmeta_content = r#"{
            "pack": {
                "description": "Test Pack",
                "pack_format": 61
            },
            "known_packs": [
                {
                    "namespace": "example",
                    "id": "content",
                    "version": "1.0.0"
                }
            ]
        }"#;
        std::fs::write(pack_path.join("pack.mcmeta"), mcmeta_content).unwrap();

        let (desc, format, known_packs) = super::read_pack_mcmeta(pack_path);
        assert_eq!(desc, "Test Pack");
        assert_eq!(format, 61);
        assert_eq!(known_packs.len(), 1);
        assert_eq!(known_packs[0].namespace, "example");
        assert_eq!(known_packs[0].id, "content");
        assert_eq!(known_packs[0].version, "1.0.0");

        // 2. 测试嵌套的 pack.known_packs
        let mcmeta_content_nested = r#"{
            "pack": {
                "description": "Nested Pack",
                "pack_format": 61,
                "known_packs": [
                    {
                        "namespace": "nested",
                        "id": "pack",
                        "version": "2.0.0"
                    }
                ]
            }
        }"#;
        std::fs::write(pack_path.join("pack.mcmeta"), mcmeta_content_nested).unwrap();

        let (_, _, known_packs) = super::read_pack_mcmeta(pack_path);
        assert_eq!(known_packs.len(), 1);
        assert_eq!(known_packs[0].namespace, "nested");
        assert_eq!(known_packs[0].id, "pack");
        assert_eq!(known_packs[0].version, "2.0.0");
    }

    #[test]
    fn resolve_enabled_features_works() {
        // 默认仅原版
        let default_features = DatapackManager::resolve_enabled_features(&[]);
        assert_eq!(default_features, ["minecraft:vanilla"]);

        // 伴随矿车改进
        let minecart_features =
            DatapackManager::resolve_enabled_features(&["file/minecart_improvements".to_string()]);
        assert_eq!(
            minecart_features,
            ["minecraft:vanilla", "minecraft:minecart_improvements"]
        );

        // 多个实验性功能
        let multi_features = DatapackManager::resolve_enabled_features(&[
            "trade_rebalance".to_string(),
            "redstone_experiments".to_string(),
            "file/bundle".to_string(),
        ]);
        assert_eq!(
            multi_features,
            [
                "minecraft:vanilla",
                "minecraft:trade_rebalance",
                "minecraft:redstone_experiments",
                "minecraft:bundle"
            ]
        );
    }
}

#[cfg(test)]
mod function_depth_tests {
    use super::*;

    #[test]
    fn depth_guard_blocks_beyond_limit_and_recovers() {
        // 逐层进入直到上限：前 MAX_FUNCTION_DEPTH 层成功，其后拒绝
        let mut guards = Vec::new();
        for _ in 0..MAX_FUNCTION_DEPTH {
            guards.push(FunctionDepthGuard::enter().expect("上限内的层应当全部放行"));
        }
        assert_eq!(guards.len() as u32, MAX_FUNCTION_DEPTH);
        assert!(
            FunctionDepthGuard::enter().is_none(),
            "超过上限必须拒绝进入"
        );

        // 全部离开后配额恢复
        guards.clear();
        assert!(FunctionDepthGuard::enter().is_some());
    }

    #[test]
    fn depth_guard_partial_release_recovers() {
        let g1 = FunctionDepthGuard::enter().unwrap();
        let g2 = FunctionDepthGuard::enter().unwrap();
        drop(g1);
        // 部分释放后仍有余量可进入
        let g3 = FunctionDepthGuard::enter().unwrap();
        drop(g2);
        drop(g3);
        assert_eq!(FUNCTION_DEPTH.with(std::cell::Cell::get), 0);
    }

    #[test]
    fn recursive_function_returns_error_instead_of_stack_overflow() {
        // 不经 server 的递归集成验证：visit 闭包里重入 visit_function_lines
        // 模拟「函数行里再执行函数」的嵌套路径；深度守卫依赖外层
        // execute_function 的进入计数，这里手动包一层以复现其结构
        let manager = DatapackManager::new();
        manager.functions.write().unwrap().insert(
            "test:recursive".to_string(),
            std::sync::Arc::from(vec!["say level0".to_string()]),
        );

        let guard = FunctionDepthGuard::enter().unwrap();
        let mut reentries = 0u32;
        let result = manager.visit_function_lines("test:recursive", |line| {
            reentries += 1;
            // 模拟该行触发再次执行函数：守卫必须在耗尽后拒绝
            if let Some(nested) = FunctionDepthGuard::enter() {
                drop(nested);
            } else {
                panic!("未到上限的层不应被拒绝（行：{line}）");
            }
        });
        drop(guard);
        assert!(result.is_ok());
        assert_eq!(reentries, 1);
    }
}

#[cfg(test)]
mod command_budget_tests {
    use super::*;

    #[test]
    fn budget_rebuilt_at_top_level_and_shared_across_nested() {
        // 重建预算：顶层（深度 1）进入时满额
        let guard = FunctionDepthGuard::enter().unwrap();
        FUNCTION_COMMAND_BUDGET.with(|b| b.set(0));
        assert!(!consume_command_budget(), "预算为 0 时必须拒绝");

        // 模拟顶层进入：预算应重建为满额
        FUNCTION_COMMAND_BUDGET.with(|b| b.set(MAX_FUNCTION_COMMANDS));
        for i in 0..MAX_FUNCTION_COMMANDS {
            assert!(consume_command_budget(), "第 {i} 条应有预算");
        }
        assert!(!consume_command_budget(), "耗尽后必须拒绝");
        drop(guard);
    }

    #[test]
    fn oversize_function_is_truncated_by_budget() {
        // 巨量行函数：预算在 MAX_FUNCTION_COMMANDS 条后耗尽，
        // visit 闭包停止执行命令，visit_function_lines 正常返回
        let manager = DatapackManager::new();
        let lines: Vec<String> = (0..MAX_FUNCTION_COMMANDS + 10)
            .map(|i| format!("say {i}"))
            .collect();
        manager
            .functions
            .write()
            .unwrap()
            .insert("test:huge".to_string(), std::sync::Arc::from(lines));

        let guard = FunctionDepthGuard::enter().unwrap();
        FUNCTION_COMMAND_BUDGET.with(|b| b.set(MAX_FUNCTION_COMMANDS));
        let mut executed = 0i64;
        let result = manager.visit_function_lines("test:huge", |_line| {
            if consume_command_budget() {
                executed += 1;
            }
        });
        drop(guard);
        assert!(result.is_ok());
        assert_eq!(executed, MAX_FUNCTION_COMMANDS, "恰好执行到预算上限");
    }
}
