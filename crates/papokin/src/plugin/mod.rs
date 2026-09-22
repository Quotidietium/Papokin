use arc_swap::ArcSwap;
use futures::future::join_all;
use loader::{LoaderError, PluginLoader, native::NativePluginLoader};
use notify::{EventKind, RecursiveMode, Watcher, event::ModifyKind};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, RwLock as SyncRwLock, atomic::AtomicBool},
    thread::ThreadId,
    time::Duration,
};
use thiserror::Error;
use tokio::{
    sync::{Notify, RwLock},
    task::JoinHandle,
};
use tracing::{debug, error, info, warn};

pub mod api;
pub mod cache;
pub mod loader;
/// 插件权限常量。
///
/// 插件可在元数据中请求这些权限，以访问特定的宿主功能。
pub mod permissions;

use crate::{LOGGER_IMPL, plugin::loader::wasm::WasmPluginLoader, server::Server};
pub use api::*;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 每当公开插件 API 或任何事件布局发生使旧的二进制插件不兼容的变化时，
/// 递增此版本号。
pub const PLUGIN_API_VERSION: u32 = 6;

const PLUGIN_DIR: &str = "./plugins";

/// 插件名会用作文件系统路径组件（`plugins/data/<name>/`）、
/// 权限命名空间（`<name>:<node>`）以及注册表键。拒绝
/// 可能越出数据目录或破坏上述用途的名称。
fn is_valid_plugin_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// 用于动态处理事件的 trait。
///
/// 该 trait 允许处理任何实现了 `Event` trait 的类型的的事件。
pub trait DynEventHandler: Send + Sync {
    /// 异步处理一个动态事件。
    ///
    /// # Arguments
    /// - `event`：待处理事件的引用。
    fn handle_dyn<'a>(
        &'a self,
        _server: &'a Arc<Server>,
        event: &'a (dyn Payload + Send + Sync),
    ) -> BoxFuture<'a, ()>;

    /// 异步处理一个阻塞式动态事件。
    ///
    /// # Arguments
    /// - `event`：待处理事件的可变引用。
    fn handle_blocking_dyn<'a>(
        &'a self,
        _server: &'a Arc<Server>,
        _event: &'a mut (dyn Payload + Send + Sync),
    ) -> BoxFuture<'a, ()>;

    /// 检查该事件处理器是否为阻塞式。
    ///
    /// # Returns
    /// 表示处理器是否阻塞的布尔值。
    fn is_blocking(&self) -> bool;

    /// 获取该事件处理器的优先级。
    ///
    /// # Returns
    /// 事件处理器的优先级。
    fn get_priority(&self) -> &EventPriority;

    /// 该处理器是否选择不接收已取消的事件（Bukkit 的
    /// `ignoreCancelled`）。为 true 时，只要事件的取消标志已置位，
    /// 分发器就跳过该处理器。
    fn ignores_cancelled(&self) -> bool {
        false
    }

    /// 返回注册该处理器的插件（如适用）。
    fn source(&self) -> Option<&str> {
        None
    }
}

/// 与 Bukkit 兼容的分发顺序：`Lowest` 最先，`Highest` 最后；排序稳定，
/// 同一优先级内按注册顺序定先后。
fn order_handlers(handlers: &[Arc<dyn DynEventHandler>]) -> Vec<&Arc<dyn DynEventHandler>> {
    let mut ordered: Vec<&Arc<dyn DynEventHandler>> = handlers.iter().collect();
    // 按 `Reverse` 升序排序，即沿派生 enum 顺序反向遍历。
    ordered.sort_by_key(|handler| std::cmp::Reverse(*handler.get_priority()));
    ordered
}

/// 根据事件的取消状态判断分发器是否可以调用 `handler`
/// （Bukkit 的 `ignoreCancelled` 语义）。
fn should_invoke(handler: &dyn DynEventHandler, cancelled: Option<bool>) -> bool {
    !(handler.ignores_cancelled() && cancelled == Some(true))
}

/// 用于处理特定事件的 trait。
///
/// 该 trait 允许处理某个实现了 `Event` trait 的特定类型的事件。
pub trait EventHandler<E: Payload>: Send + Sync {
    /// 异步处理一个 `E` 类型的事件。
    ///
    /// # Arguments
    /// - `event`：待处理事件的引用。
    fn handle<'a>(&'a self, _server: &'a Arc<Server>, _event: &'a E) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }

    /// 异步处理一个阻塞式的 `E` 类型事件。
    ///
    /// # Arguments
    /// - `event`：待处理事件的可变引用。
    fn handle_blocking<'a>(
        &'a self,
        _server: &'a Arc<Server>,
        _event: &'a mut E,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

/// 一个表示带类型事件处理器的结构体。
///
/// 此结构体保存对事件处理器的引用、其优先级以及它是否为阻塞式。
pub struct TypedEventHandler<E, H>
where
    E: Payload + Send + Sync + 'static,
    H: EventHandler<E> + Send + Sync,
{
    pub handler: Arc<H>,
    pub priority: EventPriority,
    pub blocking: bool,
    pub ignore_cancelled: bool,
    pub source: Option<String>,
    pub _phantom: std::marker::PhantomData<E>,
}

impl<E, H> DynEventHandler for TypedEventHandler<E, H>
where
    E: Payload + Send + Sync + 'static,
    H: EventHandler<E> + Send + Sync,
{
    /// 异步处理阻塞性的动态事件。
    fn handle_blocking_dyn<'a>(
        &'a self,
        server: &'a Arc<Server>,
        event: &'a mut (dyn Payload + Send + Sync),
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(typed_event) = <dyn Payload>::downcast_mut(event) {
                // handler.handle_blocking 调用现在返回一个 Future，我们会对其进行 await。
                self.handler.handle_blocking(server, typed_event).await;
            }
        })
    }

    /// 异步处理动态事件。
    fn handle_dyn<'a>(
        &'a self,
        server: &'a Arc<Server>,
        event: &'a (dyn Payload + Send + Sync),
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(typed_event) = <dyn Payload>::downcast_ref(event) {
                // handler.handle 调用现在返回一个 Future，我们会对其进行 await。
                self.handler.handle(server, typed_event).await;
            }
        })
    }

    /// 检查处理程序是否为阻塞式。
    fn is_blocking(&self) -> bool {
        self.blocking
    }

    /// 检索处理器的优先级。
    fn get_priority(&self) -> &EventPriority {
        &self.priority
    }

    fn ignores_cancelled(&self) -> bool {
        self.ignore_cancelled
    }

    fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }
}

/// 事件处理器映射的类型别名，其键为静态字符串
/// 而值是动态事件处理器的向量。
pub type HandlerMap = HashMap<&'static str, Vec<Arc<dyn DynEventHandler>>>;

/// 插件加载状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Loading,
    Loaded,
    /// 插件已加载但处于非活动状态（其 `on-enable` 失败，或者它被
    /// 运行时被禁用）。处理器与命令会被注销。
    Disabled(String),
    Failed(String),
}

/// 服务器级服务注册表中的一个条目（Bukkit `ServicesManager`）。
///
/// 服务按名称在各插件之间发现并通过 IPC 调用；原生
/// 插件还可以额外暴露类型化的进程内负载。
#[derive(Debug, Clone)]
pub struct ServiceRegistration {
    /// 服务能力名称（惯例为 `plugin:service`）。
    pub service: String,
    /// 提供该服务的插件。
    pub plugin: String,
    /// 多个提供者之间的优先级；数值越高越优先。
    pub priority: i32,
    /// 单调递增序列，用作相同优先级时的稳定次序决胜依据。
    pub sequence: u64,
}

/// 带类型的进程内服务：`name -> (owning plugin, payload)`。
type ServiceMap = Arc<RwLock<HashMap<String, (String, Arc<dyn Payload>)>>>;

/// 核心插件管理系统
pub struct PluginManager {
    plugins: SyncRwLock<Vec<LoadedPlugin>>,
    loaders: RwLock<Vec<Arc<dyn PluginLoader>>>,
    handlers: Arc<ArcSwap<HandlerMap>>,
    unloaded_files: RwLock<HashSet<PathBuf>>,
    services: ServiceMap,
    /// 跨插件服务注册表（为原生和 wasm……提供基于名称的发现）
    /// 插件同样适用）。
    service_registry: SyncRwLock<Vec<ServiceRegistration>>,
    service_sequence: std::sync::atomic::AtomicU64,
    /// 入站插件消息通道：`channel -> 处理它的插件`。
    incoming_channels: SyncRwLock<HashMap<String, Vec<String>>>,
    // 插件状态跟踪
    plugin_states: RwLock<HashMap<String, PluginState>>,
    // 插件状态变更通知
    state_notify: Arc<Notify>,
    // 用于热重载的后台任务
    hot_reload_task: RwLock<Option<JoinHandle<()>>>,
    hot_reload_enabled: AtomicBool,
    // 创建插件管理器所在线程的线程 ID。
    // 权限提示使用 rustyline，它仅在此线程上是安全的。
    main_thread_id: ThreadId,
}

/// 表示一个成功加载的插件
///
/// 操作系统相关的问题
/// - Windows：插件无法卸载，只能处于激活或未激活状态
struct LoadedPlugin {
    metadata: PluginMetadata,
    instance: Option<Arc<dyn Plugin>>,
    loader: Arc<dyn PluginLoader>,
    loader_data: Option<Box<dyn Any + Send + Sync>>,
    is_active: bool,
    context: Arc<Context>,
    path: PathBuf,
}

/// 插件管理相关的错误类型
#[derive(Error, Debug)]
pub enum ManagerError {
    #[error("未找到插件：{0}")]
    PluginNotFound(String),
    #[error("加载器错误：{0}")]
    LoaderError(#[from] LoaderError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("依赖错误：{0}")]
    DependencyError(String),
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new(true)
    }
}

impl PluginManager {
    /// 使用默认加载器创建新的插件管理器
    #[must_use]
    pub fn new(verify_plugin_signatures: bool) -> Self {
        Self {
            plugins: SyncRwLock::new(Vec::new()),
            loaders: RwLock::new(vec![
                Arc::new(NativePluginLoader),
                Arc::new(WasmPluginLoader::new(verify_plugin_signatures)),
            ]),
            handlers: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            unloaded_files: RwLock::new(HashSet::new()),
            services: Arc::new(RwLock::new(HashMap::new())),
            service_registry: SyncRwLock::new(Vec::new()),
            service_sequence: std::sync::atomic::AtomicU64::new(0),
            incoming_channels: SyncRwLock::new(HashMap::new()),
            plugin_states: RwLock::new(HashMap::new()),
            state_notify: Arc::new(Notify::new()),
            hot_reload_task: RwLock::new(None),
            hot_reload_enabled: AtomicBool::new(false),
            main_thread_id: std::thread::current().id(),
        }
    }

    /// 卸载所有已加载的插件
    pub async fn unload_all_plugins(&self) -> Result<(), ManagerError> {
        let mut plugin_names: Vec<String> = {
            let plugins = self
                .plugins
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            plugins
                .iter()
                .filter(|p| p.instance.is_some())
                .map(|p| p.metadata.name.clone())
                .collect()
        };

        // 按加载的相反顺序卸载，使依赖者先于被依赖者关停
        // 它们所依赖的插件（列表按拓扑加载顺序排列）。
        plugin_names.reverse();

        for name in plugin_names {
            if let Err(e) = self.unload_plugin(&name).await {
                error!("卸载插件 {name} 失败：{e}");
            }
        }

        Ok(())
    }

    /// 添加一个新的插件加载器实现
    pub async fn add_loader(self: &Arc<Self>, server: &Arc<Server>, loader: Arc<dyn PluginLoader>) {
        self.loaders.write().await.push(loader);

        // 尝试用新加载器加载之前已卸载的文件
        self.retry_unloaded_files(server).await;
    }

    /// 开始监视插件目录的变更
    pub async fn start_watcher(self: &Arc<Self>, server: &Arc<Server>) -> Result<(), ManagerError> {
        if self.hot_reload_task.read().await.is_some() {
            return Ok(());
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let mut watcher = notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        })
        .map_err(|e| ManagerError::IoError(std::io::Error::other(e)))?;

        let plugin_dir = Path::new(PLUGIN_DIR);
        if !plugin_dir.exists() {
            std::fs::create_dir_all(plugin_dir)?;
        }

        watcher
            .watch(plugin_dir, RecursiveMode::NonRecursive)
            .map_err(|e| ManagerError::IoError(std::io::Error::other(e)))?;

        let manager = self.clone();
        let server_clone = Arc::clone(server);
        let task = server.spawn_task(async move {
            // 一次文件保存会产生一连串通知事件（创建 + 数据
            // 修改 + 重命名）；把每一阵变更合并为每个路径一次重载
            // 改为在一段静默窗口之后进行，而不是每次卸载都完整执行一遍 unload+JIT-load
            // 事件。
            const HOT_RELOAD_DEBOUNCE: Duration = Duration::from_millis(500);

            // 通过将 watcher 移入任务来保活
            let _watcher = watcher;

            let is_reload_event = |kind: &EventKind| {
                matches!(
                    kind,
                    EventKind::Modify(ModifyKind::Data(_)) | EventKind::Create(_)
                )
            };
            let is_plugin_file = |path: &Path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"))
            };

            while let Some(event) = rx.recv().await {
                if !manager.is_hot_reload_enabled() {
                    continue;
                }
                let mut pending: HashSet<PathBuf> = if is_reload_event(&event.kind) {
                    event
                        .paths
                        .into_iter()
                        .filter(|p| is_plugin_file(p))
                        .collect()
                } else {
                    HashSet::new()
                };

                // 排空这批突发事件，直到目录在
                // 去抖窗口（或监视器消失）。
                while let Ok(Some(event)) =
                    tokio::time::timeout(HOT_RELOAD_DEBOUNCE, rx.recv()).await
                {
                    if is_reload_event(&event.kind) {
                        pending.extend(event.paths.into_iter().filter(|p| is_plugin_file(p)));
                    }
                }

                if !manager.is_hot_reload_enabled() {
                    continue;
                }
                for path in pending {
                    debug!("检测到插件变更：{:?}", path);

                    // 我们需要查明该插件是否已加载，以便先卸载它
                    let plugin_name = {
                        let plugins = manager
                            .plugins
                            .read()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        plugins
                            .iter()
                            .find(|p| p.path == path)
                            .map(|p| p.metadata.name.clone())
                    };

                    if let Some(name) = plugin_name {
                        info!("正在热重载插件：{}", name);
                        let _ = manager.unload_plugin(&name).await;
                    }

                    // 目前我们只是尝试加载它。如果它已经加载，
                    // 加载器可能会处理它，否则可能出现重复。
                    // 大多数 WASM 加载器只会创建一个新实例。
                    if let Err(e) = manager.start_loading_plugin(&server_clone, &path).await {
                        error!("热重载插件 {:?} 失败：{}", path, e);
                    }
                }
            }
        });

        *self.hot_reload_task.write().await = Some(task);
        self.set_hot_reload_enabled(true);
        Ok(())
    }

    /// 停止监视插件目录的变更
    pub async fn stop_watcher(&self) {
        let mut task_lock = self.hot_reload_task.write().await;
        if let Some(handle) = task_lock.take() {
            handle.abort();
        }
        self.set_hot_reload_enabled(false);
    }

    pub fn set_hot_reload_enabled(&self, enabled: bool) {
        self.hot_reload_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_hot_reload_enabled(&self) -> bool {
        self.hot_reload_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 重试加载先前未能加载的文件
    async fn retry_unloaded_files(self: &Arc<Self>, server: &Arc<Server>) {
        let files_to_retry: Vec<PathBuf> =
            { self.unloaded_files.read().await.iter().cloned().collect() };
        let mut retry_tasks = Vec::new();

        for path in files_to_retry {
            if let Ok(task) = self.start_loading_plugin(server, &path).await {
                retry_tasks.push(task);
            }
        }

        // 等待所有重试任务完成
        join_all(retry_tasks).await;
    }

    /// 获取加载器的克隆以供上下文使用
    #[must_use]
    pub async fn get_loaders(&self) -> Vec<Arc<dyn PluginLoader>> {
        self.loaders.read().await.clone()
    }

    /// 基于依赖关系对插件进行拓扑排序的辅助工具
    fn topological_sort(plugins: &[(String, Vec<String>)]) -> Result<Vec<String>, String> {
        fn visit(
            name: &str,
            deps_map: &HashMap<String, &Vec<String>>,
            plugin_names: &HashSet<String>,
            visited: &mut HashSet<String>,
            current_path: &mut HashSet<String>,
            sorted: &mut Vec<String>,
        ) -> Result<(), String> {
            if current_path.contains(name) {
                return Err(format!("检测到涉及插件 {name} 的循环依赖"));
            }
            if !visited.contains(name) {
                current_path.insert(name.to_string());
                if let Some(deps) = deps_map.get(name) {
                    for dep in *deps {
                        if !plugin_names.contains(dep) {
                            return Err(format!("插件 {name} 依赖缺失的插件：{dep}"));
                        }
                        visit(dep, deps_map, plugin_names, visited, current_path, sorted)?;
                    }
                }
                current_path.remove(name);
                visited.insert(name.to_string());
                sorted.push(name.to_string());
            }
            Ok(())
        }
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut current_path = HashSet::new();
        let plugin_names: HashSet<String> = plugins.iter().map(|(n, _)| n.clone()).collect();
        let deps_map: HashMap<String, &Vec<String>> =
            plugins.iter().map(|(n, d)| (n.clone(), d)).collect();

        for (name, _) in plugins {
            visit(
                name,
                &deps_map,
                &plugin_names,
                &mut visited,
                &mut current_path,
                &mut sorted,
            )?;
        }

        Ok(sorted)
    }

    /// 询问服务器所有者是否允许插件所请求的权限
    ///
    /// 此方法只能在主线程中调用，因为底层
    /// `rustyline` 控制台句柄既不是 `Send` 也不是 `Sync`。从一个
    /// 不同的线程（例如热重载监视任务）上会 panic。
    ///
    /// 返回 `(allowed, wait_time, cacheable)`。非主线程的拒绝不会被
    /// 可缓存的，以便用户在下次冷加载时再次收到提示。
    #[expect(clippy::print_stdout)]
    fn ask_permission_confirmation(
        &self,
        metadata: &PluginMetadata,
    ) -> (bool, std::time::Duration, bool) {
        use colored::Colorize;

        if metadata.permissions.is_empty() {
            return (true, std::time::Duration::ZERO, true);
        }

        if std::thread::current().id() != self.main_thread_id {
            warn!(
                "插件 \"{}\"（{}）从非主线程请求权限。\
                 权限确认提示仅在主线程上受支持。\
                 正在拒绝权限。若要以交互方式加载此插件，请重启服务器。",
                metadata.name, metadata.version
            );
            return (false, Duration::ZERO, false);
        }

        let start_time = std::time::Instant::now();

        println!(
            "\n{} \"{}\"（{}）请求以下权限：",
            "Plugin".bold(),
            metadata.name.cyan(),
            metadata.version.green()
        );
        for permission in &metadata.permissions {
            println!(
                "  - {}: {}",
                permission.yellow().bold(),
                permissions::get_permission_description(permission)
                    .unwrap_or("<未知权限>")
                    .italic()
            );
        }

        let prompt = format!("\n{} [y/N]: ", "要允许这些权限并加载该插件吗？".bold());

        let mut rl_taken = if let Some(logger_option) = crate::LOGGER_IMPL.get()
            && let Some((wrapper, _, _)) = logger_option
            && let Some(rl) = wrapper.take_readline()
        {
            Some((wrapper, rl))
        } else {
            None
        };

        let result = if let Some((_, ref mut rl)) = rl_taken {
            rl.readline(&prompt).is_ok_and(|line| {
                let input = line.trim().to_lowercase();
                input == "y" || input == "yes"
            })
        } else {
            warn!(
                "控制台 readline 不可用；无法就插件 \"{}\" 的权限进行提示",
                metadata.name
            );
            false
        };

        if let Some((wrapper, rl)) = rl_taken {
            wrapper.return_readline(rl);
        }

        (result, start_time.elapsed(), true)
    }

    /// 单个插件的生成初始化
    #[allow(clippy::too_many_lines)]
    async fn spawn_plugin_initialization(
        self: &Arc<Self>,
        server: Arc<Server>,
        instance: Arc<dyn Plugin>,
        metadata: PluginMetadata,
        loader_data: Box<dyn Any + Send + Sync>,
        loader: Arc<dyn PluginLoader>,
        path: PathBuf,
    ) -> Result<tokio::task::JoinHandle<()>, ManagerError> {
        // 将插件标记为加载中
        self.plugin_states
            .write()
            .await
            .insert(metadata.name.clone(), PluginState::Loading);

        let context = Arc::new(Context::new(
            metadata.clone(),
            server.clone(),
            Arc::clone(&self.handlers),
            Arc::clone(self),
            Arc::clone(&LOGGER_IMPL),
        ));

        // 先创建插件结构
        let plugin = LoadedPlugin {
            metadata: metadata.clone(),
            instance: None, // 将在初始化成功后被设置
            loader: loader.clone(),
            loader_data: Some(loader_data),
            is_active: false, // 将在初始化成功后被设为 true
            context: context.clone(),
            path,
        };

        self.plugins
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(plugin);

        // 为插件初始化生成异步任务
        let self_ref_clone = Arc::clone(self);
        let state_notify = Arc::clone(&self.state_notify);
        let plugin_name = metadata.name.clone();
        let loader_clone = loader.clone();

        // 下面的插件按名称查找，绝不按向量位置查找：
        // 并发卸载（如热重载）会移动索引，这可能
        // 否则会更新或移除错误的插件。
        let task = server.spawn_task(async move {
            // 初始化插件
            match instance.on_load(context.clone()).await {
                Ok(()) => {
                    // 启用分级（Paper 风格）：启用失败会使其停用
                    // 该插件（处理器/命令已注销），但保留其
                    // 已加载，而加载失败则会将其完全卸载。
                    match instance.on_enable(context.clone()).await {
                        Ok(()) => {
                            // 将插件状态更新为已加载
                            {
                                let mut plugins = self_ref_clone
                                    .plugins
                                    .write()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                                if let Some(plugin) =
                                    plugins.iter_mut().find(|p| p.metadata.name == plugin_name)
                                {
                                    plugin.instance = Some(instance);
                                    plugin.is_active = true;
                                }
                            }
                            self_ref_clone
                                .plugin_states
                                .write()
                                .await
                                .insert(plugin_name.clone(), PluginState::Loaded);
                            state_notify.notify_waiters();

                            info!("已加载 {} ({})", metadata.name, metadata.version);

                            if !metadata.permissions.is_empty() {
                                warn!(
                                    "插件 \"{}\" 使用以下权限：{:?}",
                                    metadata.name, metadata.permissions
                                );
                            }
                        }
                        Err(enable_error) => {
                            let error_msg = format!("启用失败：{enable_error}");
                            self_ref_clone.unregister_handlers(&plugin_name);
                            context.unregister_commands();

                            // 与 Paper 对齐：onEnable 失败后会
                            // onDisable，它还会停止该插件的所有任务
                            // 在其部分启用期间被调度。
                            let _ = instance.on_disable(context.clone()).await;

                            {
                                let mut plugins = self_ref_clone
                                    .plugins
                                    .write()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                                if let Some(plugin) =
                                    plugins.iter_mut().find(|p| p.metadata.name == plugin_name)
                                {
                                    plugin.instance = Some(instance);
                                    plugin.is_active = false;
                                }
                            }
                            self_ref_clone.plugin_states.write().await.insert(
                                plugin_name.clone(),
                                PluginState::Disabled(error_msg.clone()),
                            );
                            state_notify.notify_waiters();

                            error!("启用插件 {plugin_name} 失败：{error_msg}");
                        }
                    }
                }
                Err(e) => {
                    // 处理初始化失败
                    let error_msg = format!("初始化失败：{e}");
                    let _ = instance.on_unload(context).await;

                    // 在移除插件前获取加载器数据
                    let loader_data: Option<Box<dyn Any + Send + Sync>> = {
                        let mut plugins = self_ref_clone
                            .plugins
                            .write()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        plugins
                            .iter_mut()
                            .find(|p| p.metadata.name == plugin_name)
                            .and_then(|plugin| plugin.loader_data.take())
                    };

                    // 尝试卸载插件数据
                    if let Some(data) = loader_data {
                        loader_clone.unload(data).await.ok();
                    }

                    {
                        let mut plugins = self_ref_clone
                            .plugins
                            .write()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if let Some(index) =
                            plugins.iter().position(|p| p.metadata.name == plugin_name)
                        {
                            plugins.remove(index);
                        }
                    };
                    self_ref_clone
                        .plugin_states
                        .write()
                        .await
                        .insert(plugin_name.clone(), PluginState::Failed(error_msg.clone()));
                    state_notify.notify_waiters();

                    error!("初始化插件 {plugin_name} 失败：{error_msg}",);
                }
            }
        });

        Ok(task)
    }

    /// 为给定的启动阶段从插件目录加载所有插件。
    ///
    /// 元数据声明了不同 [`LoadOrder`] 的插件将被留给
    /// 各自阶段的调用，且已加载的插件绝不会被加载两次。
    #[allow(clippy::too_many_lines)]
    pub async fn load_plugins(
        self: &Arc<Self>,
        server: &Arc<Server>,
        phase: crate::plugin::api::LoadOrder,
    ) -> Result<std::time::Duration, ManagerError> {
        let path = Path::new(PLUGIN_DIR);

        if !path.exists() {
            std::fs::create_dir(path)?;
            return Ok(std::time::Duration::ZERO);
        }

        let cache_path = path.join("permission_cache.json");
        let mut cache = cache::PermissionCache::load(&cache_path).await;

        let mut prepared_plugins = Vec::new();
        let mut prepared_names: HashSet<String> = HashSet::new();
        let loaders = self.loaders.read().await.clone();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                continue;
            }

            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("deactivated"))
            {
                continue;
            }

            // 找到能处理此文件的加载器
            let mut loader_found = false;
            for loader in &loaders {
                if loader.can_load(&path) {
                    match loader.load(&path).await {
                        Ok((instance, metadata, loader_data)) => {
                            if !is_valid_plugin_name(&metadata.name) {
                                error!(
                                    "{:?} 处的插件声明了无效名称 {:?}\
                                     （允许：1-64 个 [a-zA-Z0-9-_.] 字符，不得含 `..`）；拒绝加载。",
                                    path, metadata.name
                                );
                                loader_found = true;
                                break;
                            }

                            if !prepared_names.insert(metadata.name.clone()) {
                                warn!(
                                    "插件名称 \"{}\" 重复（来自 {:?}）；\
                                     保留第一份并跳过这一份。",
                                    metadata.name, path
                                );
                                loader_found = true;
                                break;
                            }

                            let plugin_override =
                                server.advanced_config.plugins.overrides.get(&metadata.name);

                            if plugin_override.is_some_and(|o| !o.enabled) {
                                info!("插件 \"{}\" 已在配置中禁用，跳过。", metadata.name);
                                loader_found = true;
                                break;
                            }

                            let allow_unsigned = plugin_override
                                .and_then(|o| o.allow_unsigned)
                                .unwrap_or(server.advanced_config.plugins.allow_unsigned);

                            if !allow_unsigned
                                && path
                                    .extension()
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"))
                            {
                                let wasm_bytes = std::fs::read(&path).unwrap_or_default();
                                if !crate::plugin::loader::wasm::wasm_host::signature::is_wasm_signed(&wasm_bytes) {
                                    error!(
                                        "插件 \"{}\"（{:?}）未签名或无效，且配置中禁用了 allow_unsigned，跳过。",
                                        metadata.name, path
                                    );
                                    loader_found = true;
                                    break;
                                }
                            }

                            prepared_plugins.push((
                                instance,
                                metadata,
                                loader_data,
                                loader.clone(),
                                path.clone(),
                            ));
                            loader_found = true;
                        }
                        Err(err) => error!("从 {:?} 加载插件失败：{}", path, err),
                    }
                    break;
                }
            }

            if !loader_found {
                self.unloaded_files.write().await.insert(path.clone());
            }
        }

        // 阶段拆分：只初始化声明了此阶段的插件，并且
        // 未被更早的阶段加载。后续阶段会重新扫描相同的
        // 目录，因此必须在此处过滤掉已加载的插件。
        let already_loaded: HashSet<String> = {
            let plugins = self
                .plugins
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            plugins.iter().map(|p| p.metadata.name.clone()).collect()
        };
        prepared_plugins.retain(|(_, metadata, ..)| {
            if metadata.load_order != phase {
                return false;
            }
            if already_loaded.contains(&metadata.name) {
                info!("插件 \"{}\" 已加载，跳过重复加载。", metadata.name);
                return false;
            }
            true
        });

        // 解析依赖边：硬 `dependencies`、软 `load_after` /
        // `load_before`，以及 `provides` 能力别名（Paper 风格）。
        let plugin_names: HashSet<String> = prepared_plugins
            .iter()
            .map(|(_, m, ..)| m.name.clone())
            .chain(already_loaded.iter().cloned())
            .collect();
        let mut provides_map: HashMap<String, String> = HashMap::new();
        for (_, metadata, ..) in &prepared_plugins {
            for capability in &metadata.provides {
                if capability == &metadata.name {
                    continue;
                }
                match provides_map.get(capability) {
                    Some(existing) => warn!(
                        "插件 \"{existing}\" 和 \"{}\" 都提供 `{capability}`；使用 \"{existing}\"",
                        metadata.name
                    ),
                    None => {
                        provides_map.insert(capability.clone(), metadata.name.clone());
                    }
                }
            }
        }
        // 在本批次中将依赖名称解析为具体插件。
        let resolve_dep = |dep: &str| -> Option<String> {
            if plugin_names.contains(dep) {
                return Some(dep.to_string());
            }
            provides_map.get(dep).cloned()
        };

        // `deps[p]` 列出了必须在 `p` 之前加载的插件。
        let mut skipped: HashMap<String, String> = HashMap::new();
        let mut name_deps: Vec<(String, Vec<String>)> = Vec::new();
        for (_, metadata, ..) in &prepared_plugins {
            let mut deps: Vec<String> = Vec::new();
            let mut hard_missing = false;
            for dep in &metadata.dependencies {
                match resolve_dep(dep) {
                    Some(name) if name != metadata.name => deps.push(name),
                    Some(_) => {}
                    None => {
                        warn!("插件 \"{}\" 无法加载：缺少硬依赖 `{dep}`", metadata.name);
                        skipped.insert(metadata.name.clone(), format!("缺少硬依赖 `{dep}`"));
                        hard_missing = true;
                    }
                }
            }
            if hard_missing {
                continue;
            }
            for dep in &metadata.load_after {
                match resolve_dep(dep) {
                    Some(name) if name != metadata.name => deps.push(name),
                    _ => {} // 指向缺失插件的软边引用会被忽略
                }
            }
            name_deps.push((metadata.name.clone(), deps));
        }

        // `load_before`：`p.load_before(x)` 表示 `p` 必须在 `x` 之前加载，即
        // “必须先加载”图中的一条边 `x -> p`。
        let before_edges: Vec<(String, Vec<String>)> = prepared_plugins
            .iter()
            .map(|(_, m, ..)| {
                let targets = m
                    .load_before
                    .iter()
                    .filter_map(|dep| resolve_dep(dep))
                    .filter(|resolved| resolved != &m.name)
                    .collect();
                (m.name.clone(), targets)
            })
            .collect();
        for (source, targets) in &before_edges {
            for target in targets {
                if let Some(entry) = name_deps.iter_mut().find(|(n, _)| n == target)
                    && !entry.1.contains(source)
                {
                    entry.1.push(source.clone());
                }
            }
        }

        // 硬依赖被跳过的插件会被连带跳过。
        loop {
            let present: HashSet<String> = name_deps.iter().map(|(n, _)| n.clone()).collect();
            let mut removed = false;
            name_deps.retain(|(name, deps)| {
                let missing = deps.iter().find(|d| !present.contains(*d));
                if let Some(missing) = missing {
                    skipped.insert(name.clone(), format!("硬依赖 `{missing}` 被跳过或缺失"));
                    removed = true;
                    warn!("插件 \"{name}\" 无法加载：硬依赖 `{missing}` 被跳过或缺失");
                    return false;
                }
                true
            });
            if !removed {
                break;
            }
        }

        if !skipped.is_empty() {
            let names: Vec<String> = skipped.keys().cloned().collect();
            warn!("因依赖问题跳过的插件：{}", names.join(", "));
        }

        // 已在早期阶段解析为插件的边已满足
        // （这些插件已加载）；在排序前丢弃它们，排序只能看到
        // 本批次中的插件。
        let batch_names: HashSet<String> = name_deps.iter().map(|(n, _)| n.clone()).collect();
        for (_, deps) in &mut name_deps {
            deps.retain(|d| batch_names.contains(d));
        }

        let sorted_names =
            Self::topological_sort(&name_deps).map_err(ManagerError::DependencyError)?;

        // 将名称映射回准备好的插件
        #[expect(clippy::type_complexity)]
        let mut plugins_map: HashMap<
            String,
            (
                Arc<dyn Plugin>,
                PluginMetadata,
                Box<dyn Any + Send + Sync>,
                Arc<dyn PluginLoader>,
                PathBuf,
            ),
        > = prepared_plugins
            .into_iter()
            .map(|(i, m, d, l, p)| (m.name.clone(), (i, m, d, l, p)))
            .collect();

        let mut total_wait_time = std::time::Duration::ZERO;

        for name in sorted_names {
            if let Some((instance, metadata, loader_data, loader, path)) = plugins_map.remove(&name)
            {
                let (allowed, wait_time) = self
                    .clone()
                    .check_permissions_cached(&path, &metadata, &mut cache, &cache_path, server)
                    .await;

                total_wait_time += wait_time;

                if !allowed {
                    warn!("插件 \"{}\" 的权限被拒绝，跳过加载。", metadata.name);
                    continue;
                }

                match self
                    .spawn_plugin_initialization(
                        server.clone(),
                        instance,
                        metadata,
                        loader_data,
                        loader,
                        path,
                    )
                    .await
                {
                    Ok(task) => {
                        // 我们必须等待每次初始化完成，以确保依赖已就绪
                        if let Err(err) = task.await {
                            error!("插件初始化任务发生 panic：{}", err);
                        }
                    }
                    Err(err) => error!("{}", err),
                }
            }
        }

        // 地图中剩余的内容均被依赖解析跳过。
        for (name, (_, metadata, ..)) in plugins_map {
            warn!(
                "插件 \"{}\"（{}）已被跳过，不会被加载。",
                name, metadata.version
            );
        }

        // 该阶段的所有插件已完成加载和启用。
        // `PostWorld` 是最后的启动阶段，因此关闭注册表并标记
        // 在此处注册：更早冻结也会阻止那些
        // 在稍后阶段加载。
        if phase == crate::plugin::api::LoadOrder::PostWorld {
            server.registry_manager.freeze();
            server.tag_manager.freeze();
        }

        Ok(total_wait_time)
    }

    async fn check_permissions_cached(
        &self,
        path: &Path,
        metadata: &PluginMetadata,
        cache: &mut cache::PermissionCache,
        cache_path: &Path,
        server: &Arc<Server>,
    ) -> (bool, std::time::Duration) {
        let plugin_config = &server.advanced_config.plugins;
        let plugin_override = plugin_config.overrides.get(&metadata.name);

        let is_blocked = |p: &str| {
            plugin_config.blocked_permissions.iter().any(|b| b == p)
                || plugin_override.is_some_and(|o| o.blocked_permissions.iter().any(|b| b == p))
        };

        let is_pre_allowed = |p: &str| {
            plugin_config.allowed_permissions.iter().any(|a| a == p)
                || plugin_override.is_some_and(|o| o.allowed_permissions.iter().any(|a| a == p))
        };

        let effective_permissions: Vec<String> = metadata
            .permissions
            .iter()
            .filter(|p| !is_blocked(p))
            .cloned()
            .collect();

        // 如果所有请求的权限均已预先允许，则无需提示直接授予
        if !effective_permissions.iter().any(|p| !is_pre_allowed(p)) {
            return (true, std::time::Duration::ZERO);
        }

        let hash = cache::calculate_hash(path).await.unwrap_or_default();

        if let Some(entry) = cache.entries.get(&hash)
            && entry.permissions_requested == metadata.permissions
        {
            info!(
                "找到插件 \"{}\" 的缓存权限决定（批准：{}）",
                metadata.name, entry.approved
            );
            return (entry.approved, std::time::Duration::ZERO);
        }

        if !plugin_config.ask_permission_confirmation {
            info!(
                "自动批准插件 \"{}\" 的权限（ask_permission_confirmation 已禁用）",
                metadata.name
            );
            cache.entries.insert(
                hash,
                cache::PermissionCacheEntry {
                    permissions_requested: metadata.permissions.clone(),
                    approved: true,
                },
            );
            let _ = cache.save(cache_path).await;
            return (true, std::time::Duration::ZERO);
        }

        let (allowed, wait_time, cacheable) = self.ask_permission_confirmation(metadata);
        if cacheable {
            cache.entries.insert(
                hash,
                cache::PermissionCacheEntry {
                    permissions_requested: metadata.permissions.clone(),
                    approved: allowed,
                },
            );
            let _ = cache.save(cache_path).await;
        }
        (allowed, wait_time)
    }

    /// 异步开始加载插件
    async fn start_loading_plugin(
        self: &Arc<Self>,
        server: &Arc<Server>,
        path: &Path,
    ) -> Result<tokio::task::JoinHandle<()>, ManagerError> {
        if !server.advanced_config.plugins.enabled {
            return Err(ManagerError::LoaderError(LoaderError::RuntimeError(
                "插件系统已在配置中禁用".to_string(),
            )));
        }

        let loaders = self.loaders.read().await.clone();
        for loader in &loaders {
            if loader.can_load(path) {
                let (instance, metadata, loader_data) = loader.load(path).await?;

                if !is_valid_plugin_name(&metadata.name) {
                    return Err(ManagerError::LoaderError(LoaderError::RuntimeError(
                        format!(
                            "Plugin at {} declares an invalid name {:?} \
                             (allowed: 1-64 chars of [a-zA-Z0-9-_.], no `..`); refusing to load.",
                            path.display(),
                            metadata.name
                        ),
                    )));
                }

                let plugin_override = server.advanced_config.plugins.overrides.get(&metadata.name);

                if self.is_plugin_loaded(&metadata.name) {
                    return Err(ManagerError::LoaderError(LoaderError::RuntimeError(
                        format!("Plugin \"{}\" 已加载；请先卸载再加载新副本", metadata.name),
                    )));
                }

                if plugin_override.is_some_and(|o| !o.enabled) {
                    return Err(ManagerError::LoaderError(LoaderError::RuntimeError(
                        format!("插件 \"{}\" 已在配置中禁用", metadata.name),
                    )));
                }

                let allow_unsigned = plugin_override
                    .and_then(|o| o.allow_unsigned)
                    .unwrap_or(server.advanced_config.plugins.allow_unsigned);

                if !allow_unsigned
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"))
                {
                    let wasm_bytes = std::fs::read(path).unwrap_or_default();
                    if !crate::plugin::loader::wasm::wasm_host::signature::is_wasm_signed(
                        &wasm_bytes,
                    ) {
                        return Err(ManagerError::LoaderError(LoaderError::RuntimeError(
                            format!(
                                "插件 \"{}\" 未签名或无效，且 allow_unsigned 已禁用",
                                metadata.name
                            ),
                        )));
                    }
                }

                let cache_path = Path::new(PLUGIN_DIR).join("permission_cache.json");
                let mut cache = cache::PermissionCache::load(&cache_path).await;

                let (allowed, _) = self
                    .check_permissions_cached(path, &metadata, &mut cache, &cache_path, server)
                    .await;

                if !allowed {
                    warn!("插件 \"{}\" 的权限被拒绝，跳过加载。", metadata.name);
                    return Err(ManagerError::LoaderError(LoaderError::RuntimeError(
                        "权限被拒绝".to_string(),
                    )));
                }

                return self
                    .spawn_plugin_initialization(
                        server.clone(),
                        instance,
                        metadata,
                        loader_data,
                        loader.clone(),
                        path.to_path_buf(),
                    )
                    .await;
            }
        }

        // 没有加载器能处理此文件，将其记录下来以便日后重试
        self.unloaded_files.write().await.insert(path.to_path_buf());

        Err(ManagerError::PluginNotFound(
            path.to_string_lossy().to_string(),
        ))
    }

    /// 尝试加载单个插件文件
    pub async fn try_load_plugin(
        self: &Arc<Self>,
        server: &Arc<Server>,
        path: &Path,
    ) -> Result<(), ManagerError> {
        self.start_loading_plugin(server, path)
            .await?
            .await
            .map_err(|e| {
                ManagerError::LoaderError(LoaderError::InitializationFailed(format!(
                    "任务 join 错误：{e}"
                )))
            })
    }

    /// 等待某个插件完成加载
    pub async fn wait_for_plugin(&self, plugin_name: &str) -> Result<(), ManagerError> {
        loop {
            // 在重新读取状态*之前*注册为等待者；否则
            // `notify_waiters` 若恰好落在读取与 await 之间，就会
            // 错过，此循环将永远挂起。
            let notified = self.state_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            let state = self.plugin_states.read().await.get(plugin_name).cloned();
            let Some(state) = state else {
                return Err(ManagerError::PluginNotFound(plugin_name.to_string()));
            };
            match state {
                PluginState::Loaded => return Ok(()),
                PluginState::Disabled(error) | PluginState::Failed(error) => {
                    return Err(ManagerError::LoaderError(
                        LoaderError::InitializationFailed(error),
                    ));
                }
                PluginState::Loading => {
                    notified.await;
                }
            }
        }
    }

    /// 获取插件的当前状态
    pub async fn get_plugin_state(&self, plugin_name: &str) -> Option<PluginState> {
        self.plugin_states.read().await.get(plugin_name).cloned()
    }

    /// 检查插件是否处于活动状态
    #[must_use]
    pub fn is_plugin_active(&self, name: &str) -> bool {
        let plugins = self
            .plugins
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        plugins
            .iter()
            .any(|p| p.metadata.name == name && p.is_active && p.instance.is_some())
    }

    ///当插件处于 `on_load` 与对应生命周期结束之间时，返回 `true`
    /// `on_enable`——即其注册项必须已经可被观察到。
    fn is_plugin_loading(&self, name: &str) -> bool {
        self.plugin_states
            .try_read()
            .is_ok_and(|states| matches!(states.get(name), Some(PluginState::Loading)))
    }

    /// 获取活跃插件的列表
    #[must_use]
    pub fn active_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self
            .plugins
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        plugins
            .iter()
            .filter(|p| p.is_active && p.instance.is_some())
            .map(|p| p.metadata.clone())
            .collect()
    }

    /// 检查插件是否已加载
    #[must_use]
    pub fn is_plugin_loaded(&self, name: &str) -> bool {
        let plugins = self
            .plugins
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        plugins.iter().any(|p| p.metadata.name == name)
    }

    /// 获取已加载插件的列表
    #[must_use]
    pub fn loaded_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self
            .plugins
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        plugins.iter().map(|p| p.metadata.clone()).collect()
    }

    /// 按名称卸载插件
    pub async fn unload_plugin(&self, name: &str) -> Result<(), ManagerError> {
        let mut plugin = {
            let mut plugins = self
                .plugins
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let index = plugins
                .iter()
                .position(|p| p.metadata.name == name)
                .ok_or_else(|| ManagerError::PluginNotFound(name.to_string()))?;
            plugins.remove(index)
        };

        self.unregister_handlers(name);
        plugin.context.unregister_commands();
        self.unregister_all_service_providers(name);
        self.unregister_all_incoming_channels(name);

        if let Some(instance) = plugin.instance.take() {
            // 活动插件在卸载前会被优雅地禁用。
            if plugin.is_active {
                instance.on_disable(plugin.context.clone()).await.ok();
            }
            instance.on_unload(plugin.context.clone()).await.ok();
        }

        if plugin.loader.can_unload() {
            let unload_result = match plugin.loader_data {
                Some(data) => plugin.loader.unload(data).await,
                None => Ok(()),
            };
            // 无论如何，从管理器的角度看插件都已不复存在；
            // 在加载器失败时绝不会留下过期的 `Loaded` 状态。
            self.plugin_states.write().await.remove(name);
            unload_result?;
        } else {
            plugin.is_active = false;
            self.plugins
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(plugin);
            self.plugin_states.write().await.remove(name);
        }

        Ok(())
    }

    /// 禁用已加载的插件，但不卸载它。
    ///
    /// 注销其事件处理器与命令，并调用 `on_disable`。
    /// 对应 Paper 的插件禁用：插件保持已加载状态，以便
    /// 仍可查看，但不再参与服务器运行。
    pub async fn disable_plugin(&self, name: &str) -> Result<(), ManagerError> {
        let (instance, context, is_active) = {
            let plugins = self
                .plugins
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let plugin = plugins
                .iter()
                .find(|p| p.metadata.name == name)
                .ok_or_else(|| ManagerError::PluginNotFound(name.to_string()))?;
            (
                plugin.instance.clone(),
                plugin.context.clone(),
                plugin.is_active,
            )
        };

        let Some(instance) = instance else {
            return Err(ManagerError::PluginNotFound(name.to_string()));
        };
        if !is_active {
            return Ok(()); // 已禁用
        }

        self.unregister_handlers(name);
        context.unregister_commands();
        self.unregister_all_service_providers(name);
        self.unregister_all_incoming_channels(name);
        instance.on_disable(context).await.ok();

        {
            let mut plugins = self
                .plugins
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(plugin) = plugins.iter_mut().find(|p| p.metadata.name == name) {
                plugin.is_active = false;
            }
        }
        self.plugin_states.write().await.insert(
            name.to_string(),
            PluginState::Disabled("已被服务器禁用".to_string()),
        );
        info!("已禁用插件 {name}");
        Ok(())
    }

    fn unregister_handlers(&self, source: &str) {
        self.handlers.rcu(|handlers| {
            let mut new_handlers = (**handlers).clone();
            new_handlers.retain(|_, handlers| {
                handlers.retain(|handler| handler.source() != Some(source));
                !handlers.is_empty()
            });
            Arc::new(new_handlers)
        });
    }

    /// 获取所有正在加载的插件
    pub async fn get_loading_plugins(&self) -> Vec<String> {
        let plugin_states = self.plugin_states.read().await;
        plugin_states
            .iter()
            .filter(|(_, state)| matches!(state, PluginState::Loading))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// 获取所有加载失败的插件
    pub async fn get_failed_plugins(&self) -> Vec<(String, String)> {
        let plugin_states = self.plugin_states.read().await;
        plugin_states
            .iter()
            .filter_map(|(name, state)| {
                if let PluginState::Failed(error) = state {
                    Some((name.clone(), error.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// 检查所有插件是否已加载完毕（无论成功或失败）
    pub async fn all_plugins_loaded(&self) -> bool {
        let plugin_states = self.plugin_states.read().await;
        !plugin_states
            .values()
            .any(|state| matches!(state, PluginState::Loading))
    }

    /// 等待所有插件完成加载
    pub async fn wait_for_all_plugins(&self) {
        loop {
            // 与 `wait_for_plugin` 相同的避免唤醒丢失机制。
            let notified = self.state_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            if self.all_plugins_loaded().await {
                return;
            }
            notified.await;
        }
    }

    /// 注册一个事件处理器
    pub fn register<E, H>(
        &self,
        handler: Arc<H>,
        priority: EventPriority,
        blocking: bool,
        ignore_cancelled: bool,
    ) where
        E: Payload + Send + Sync + 'static,
        H: EventHandler<E> + 'static,
    {
        let typed_handler = Arc::new(TypedEventHandler {
            handler,
            priority,
            blocking,
            ignore_cancelled,
            source: None,
            _phantom: std::marker::PhantomData,
        });

        self.handlers.rcu(|handlers| {
            let mut new_handlers = (**handlers).clone();
            new_handlers
                .entry(E::get_name_static())
                .or_default()
                .push(typed_handler.clone());
            Arc::new(new_handlers)
        });
    }

    #[must_use]
    pub fn has_handlers<E: Payload + 'static>(&self) -> bool {
        self.handlers
            .load()
            .get(E::get_name_static())
            .is_some_and(|handlers| !handlers.is_empty())
    }

    /// 向所有已注册的处理器触发事件
    ///
    /// 处理器按与 Bukkit 兼容的优先级顺序运行（`Lowest` 最先，
    /// `Highest` 最后，同一优先级内按注册顺序）；所有阻塞式
    /// 处理程序会先于任何非阻塞处理程序运行。选择退出的处理程序
    /// 已取消事件（`ignoreCancelled`）会被跳过，而事件的
    /// 取消标志已被设置。
    pub async fn fire<E: Payload + Send + Sync + 'static>(
        &self,
        server: &Arc<Server>,
        event: &mut E,
    ) {
        let handlers_map = self.handlers.load();
        if handlers_map.is_empty() {
            return;
        }

        let Some(handlers) = handlers_map.get(E::get_name_static()) else {
            return;
        };

        if handlers.is_empty() {
            return;
        }

        let ordered = order_handlers(handlers);

        for phase in [true, false] {
            for handler in &ordered {
                if handler.is_blocking() != phase {
                    continue;
                }
                // 每个处理器重新读取取消标志：较早的阻塞
                // 处理函数可能在派发中途取消了事件，而后续
                // `ignoreCancelled` 处理器必须观察到这一点（Bukkit 语义）。
                if !should_invoke(handler.as_ref(), event.cancelled_state()) {
                    continue;
                }
                if phase {
                    handler.handle_blocking_dyn(server, event).await;
                } else {
                    handler.handle_dyn(server, event).await;
                }
            }
        }
    }

    /// 同步向所有已注册的处理器触发事件（若存在处理器则阻塞）。
    /// 若此事件未注册任何处理器，则立即返回，不产生运行时开销。
    pub fn fire_blocking<E: Payload + Send + Sync + 'static>(
        &self,
        server: &Arc<Server>,
        event: &mut E,
    ) {
        let handlers_map = self.handlers.load();
        if handlers_map.is_empty() {
            return;
        }

        let Some(handlers) = handlers_map.get(E::get_name_static()) else {
            return;
        };

        if handlers.is_empty() {
            return;
        }

        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| {
                server.runtime.block_on(self.fire(server, event));
            });
        } else {
            server.runtime.block_on(self.fire(server, event));
        }
    }

    #[expect(clippy::result_unit_err)]
    pub async fn send_message(
        &self,
        sender: &str,
        recipient: &str,
        message: &[u8],
    ) -> Result<Result<Vec<u8>, String>, ()> {
        if sender == recipient {
            return Err(());
        }

        let instance = {
            let plugins = self
                .plugins
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let target_plugin = plugins
                .iter()
                .find(|p| p.metadata.name == recipient)
                .ok_or(())?;
            // 已禁用的插件不再参与服务器；加载
            // 插件可以应答（其服务已可被发现）。
            if !target_plugin.is_active && !self.is_plugin_loading(recipient) {
                return Err(());
            }
            target_plugin.instance.clone()
        };
        if let Some(instance) = instance {
            Ok(instance.on_ipc_message(sender, message).await)
        } else {
            Err(())
        }
    }

    /// 以给定优先级将 `plugin` 注册为 `service` 的提供者。
    ///
    /// 重新注册相同的（service, plugin）组合会更新其优先级；
    /// 原始注册顺序被保留作为平局裁决依据。
    pub fn register_service_provider(&self, service: &str, plugin: &str, priority: i32) {
        let sequence = self
            .service_sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut registry = self
            .service_registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = registry
            .iter_mut()
            .find(|r| r.service == service && r.plugin == plugin)
        {
            existing.priority = priority;
            return;
        }
        registry.push(ServiceRegistration {
            service: service.to_string(),
            plugin: plugin.to_string(),
            priority,
            sequence,
        });
    }

    /// 移除 `plugin` 对 `service` 的注册。
    pub fn unregister_service_provider(&self, service: &str, plugin: &str) {
        self.service_registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|r| !(r.service == service && r.plugin == plugin));
    }

    /// 移除 `plugin` 进行的所有服务注册。
    pub fn unregister_all_service_providers(&self, plugin: &str) {
        self.service_registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|r| r.plugin != plugin);
    }

    ///返回 `service` 的所有活动提供者，按优先级降序排序
    /// 优先级排序（其次按注册顺序）。
    #[must_use]
    pub fn get_service_providers(&self, service: &str) -> Vec<ServiceRegistration> {
        let registry = self
            .service_registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut providers: Vec<ServiceRegistration> = registry
            .iter()
            .filter(|r| {
                r.service == service
                    // 处于启用过程中的插件（状态 `Loading`）已被允许
                    // 提供服务——其注册项必须可被发现
                    // 立即执行，包括从其自身的 on_enable 中触发。
                    && (self.is_plugin_active(&r.plugin) || self.is_plugin_loading(&r.plugin))
            })
            .cloned()
            .collect();
        providers.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.sequence.cmp(&b.sequence))
        });
        providers
    }

    /// 将 `plugin` 注册为传入消息通道的处理器。
    ///
    /// 保留的 `minecraft:*` 通道无法注册。注册
    /// 相同的（channel, plugin）组合两次是无操作。
    pub fn register_incoming_channel(&self, channel: &str, plugin: &str) -> Result<(), String> {
        if channel.starts_with("minecraft:") {
            return Err(format!("通道 `{channel}` 为保留通道（minecraft:*）"));
        }
        let mut channels = self
            .incoming_channels
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let handlers = channels.entry(channel.to_string()).or_default();
        if !handlers.iter().any(|p| p == plugin) {
            handlers.push(plugin.to_string());
        }
        Ok(())
    }

    /// 将 `plugin` 从 `channel` 的处理器中移除。
    pub fn unregister_incoming_channel(&self, channel: &str, plugin: &str) {
        let mut channels = self
            .incoming_channels
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(handlers) = channels.get_mut(channel) {
            handlers.retain(|p| p != plugin);
            if handlers.is_empty() {
                channels.remove(channel);
            }
        }
    }

    /// 移除 `plugin` 进行的所有通道注册。
    pub fn unregister_all_incoming_channels(&self, plugin: &str) {
        let mut channels = self
            .incoming_channels
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        channels.retain(|_, handlers| {
            handlers.retain(|p| p != plugin);
            !handlers.is_empty()
        });
    }

    ///返回 `plugin` 注册的通道。
    #[must_use]
    pub fn get_plugin_channels(&self, plugin: &str) -> Vec<String> {
        let channels = self
            .incoming_channels
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut owned: Vec<String> = channels
            .iter()
            .filter(|(_, handlers)| handlers.iter().any(|p| p == plugin))
            .map(|(channel, _)| channel.clone())
            .collect();
        owned.sort();
        owned
    }

    ///返回处理 `channel` 的活动插件（按注册顺序）。
    #[must_use]
    pub fn get_channel_handlers(&self, channel: &str) -> Vec<String> {
        let channels = self
            .incoming_channels
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        channels
            .get(channel)
            .map(|handlers| {
                handlers
                    .iter()
                    .filter(|p| self.is_plugin_active(p))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 将客户端插件消息分派给每一个已注册的活跃插件
    /// 用于 `channel`（Bukkit `Messenger` 的入站分发）。
    pub async fn dispatch_plugin_message(
        &self,
        channel: &str,
        player_uuid: uuid::Uuid,
        data: Vec<u8>,
    ) {
        let handlers = self.get_channel_handlers(channel);
        for name in handlers {
            let instance = {
                let plugins = self
                    .plugins
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                plugins
                    .iter()
                    .find(|p| p.metadata.name == name && p.is_active)
                    .and_then(|p| p.instance.clone())
            };
            if let Some(instance) = instance {
                instance
                    .on_plugin_message(player_uuid, channel, &data)
                    .await
                    .ok();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn topological_sort() {
        let plugins = vec![
            ("A".to_string(), vec!["B".to_string()]),
            ("B".to_string(), vec!["C".to_string()]),
            ("C".to_string(), vec![]),
        ];
        let sorted = PluginManager::topological_sort(&plugins).unwrap();
        assert_eq!(sorted, vec!["C", "B", "A"]);

        let plugins_complex = vec![
            ("A".to_string(), vec!["B".to_string(), "C".to_string()]),
            ("B".to_string(), vec!["D".to_string()]),
            ("C".to_string(), vec!["D".to_string()]),
            ("D".to_string(), vec![]),
        ];
        let sorted = PluginManager::topological_sort(&plugins_complex).unwrap();
        // 可能存在多种合法排序，但 D 必须在 B 和 C 之前，且 B、C 必须在 A 之前。
        assert_eq!(sorted[0], "D");
        assert!(sorted[1] == "B" || sorted[1] == "C");
        assert!(sorted[2] == "B" || sorted[2] == "C");
        assert_eq!(sorted[3], "A");

        let plugins_circular = vec![
            ("A".to_string(), vec!["B".to_string()]),
            ("B".to_string(), vec!["A".to_string()]),
        ];
        assert!(PluginManager::topological_sort(&plugins_circular).is_err());

        let plugins_missing = vec![("A".to_string(), vec!["B".to_string()])];
        assert!(PluginManager::topological_sort(&plugins_missing).is_err());
    }

    struct DummyHandler;

    impl EventHandler<crate::plugin::api::events::block::block_break::BlockBreakEvent>
        for DummyHandler
    {
    }

    fn dummy_handler(
        priority: EventPriority,
        blocking: bool,
        ignore_cancelled: bool,
    ) -> Arc<dyn DynEventHandler> {
        Arc::new(TypedEventHandler {
            handler: Arc::new(DummyHandler),
            priority,
            blocking,
            ignore_cancelled,
            source: None,
            _phantom: std::marker::PhantomData,
        })
    }

    #[test]
    fn bukkit_priority_order() {
        let handlers: Vec<Arc<dyn DynEventHandler>> = vec![
            dummy_handler(EventPriority::Highest, true, false),
            dummy_handler(EventPriority::Low, true, false),
            dummy_handler(EventPriority::Normal, true, false),
            dummy_handler(EventPriority::Lowest, true, false),
            dummy_handler(EventPriority::High, true, false),
        ];
        let ordered = order_handlers(&handlers);
        let priorities: Vec<_> = ordered.iter().map(|h| *h.get_priority()).collect();
        assert_eq!(
            priorities,
            vec![
                EventPriority::Lowest,
                EventPriority::Low,
                EventPriority::Normal,
                EventPriority::High,
                EventPriority::Highest,
            ]
        );
    }

    #[test]
    fn ignore_cancelled_gate() {
        let ignoring = dummy_handler(EventPriority::Normal, true, true);
        let observing = dummy_handler(EventPriority::Normal, true, false);

        // 未取消：两个处理器都会运行。
        assert!(should_invoke(ignoring.as_ref(), Some(false)));
        assert!(should_invoke(observing.as_ref(), Some(false)));
        // 已取消：ignoreCancelled 处理器被跳过，另一个则执行。
        assert!(!should_invoke(ignoring.as_ref(), Some(true)));
        assert!(should_invoke(observing.as_ref(), Some(true)));
        // 不可取消的事件永不拦截。
        assert!(should_invoke(ignoring.as_ref(), None));
    }

    #[test]
    fn cancellable_event_reports_cancellation() {
        use crate::plugin::api::events::block::block_break::BlockBreakEvent;

        let mut event = BlockBreakEvent {
            player: None,
            block: &papokin_data::Block::STONE,
            block_position: papokin_util::math::position::BlockPos::new(0, 0, 0),
            exp: 0,
            drop: true,
            cancelled: false,
        };
        assert_eq!(
            crate::plugin::api::events::Payload::cancelled_state(&event),
            Some(false)
        );
        event.cancelled = true;
        assert_eq!(
            crate::plugin::api::events::Payload::cancelled_state(&event),
            Some(true)
        );
    }
}
