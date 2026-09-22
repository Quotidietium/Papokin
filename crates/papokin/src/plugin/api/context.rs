use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use crate::{LoggerOption, command::client_suggestions, plugin::PluginMetadata, plugin_log};
use arc_swap::ArcSwap;
use papokin_util::{
    PermissionLvl,
    permission::{Permission, PermissionManager},
};
use tracing::Level;

use crate::{
    entity::player::Player,
    plugin::{EventHandler, HandlerMap, PluginManager, TypedEventHandler},
    server::Server,
};

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

use super::{EventPriority, Payload};

/// `Context` 结构体表示插件的上下文，包含元数据、
/// 一个服务器引用，以及事件处理器。
///
/// # Fields
/// - `metadata`：插件的元数据。
/// - `server`：插件所运行的服务器的引用。
/// - `handlers`：事件处理器的映射，包裹在 `ArcSwap` 中以实现跨线程的无锁读取。
pub struct Context {
    metadata: PluginMetadata,
    pub server: Arc<Server>,
    pub handlers: Arc<ArcSwap<HandlerMap>>,
    pub plugin_manager: Arc<PluginManager>,
    pub permission_manager: Arc<PermissionManager>,
    pub logger: Arc<OnceLock<LoggerOption>>,
}
impl Context {
    /// 创建新的 `Context` 实例。
    ///
    /// # Arguments
    /// - `metadata`：插件的元数据。
    /// - `server`：服务器的引用。
    /// - `handlers`：包含事件处理器的集合。
    ///
    /// # Returns
    /// 一个新的 `Context` 实例。
    #[must_use]
    pub fn new(
        metadata: PluginMetadata,
        server: Arc<Server>,
        handlers: Arc<ArcSwap<HandlerMap>>,
        plugin_manager: Arc<PluginManager>,
        logger: Arc<OnceLock<LoggerOption>>,
    ) -> Self {
        let permission_manager = server.permission_manager.clone();
        Self {
            metadata,
            server,
            handlers,
            plugin_manager,
            permission_manager,
            logger,
        }
    }

    #[must_use]
    pub const fn get_metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// 检索插件的数据文件夹路径，若不存在则创建。
    ///
    /// # Returns
    /// 一个表示数据文件夹路径的字符串。
    #[must_use]
    pub fn get_data_folder(&self) -> PathBuf {
        let path = Path::new("plugins").join("data").join(&self.metadata.name);
        if !path.exists()
            && let Err(error) = fs::create_dir_all(&path)
        {
            tracing::error!(
                "为插件 \"{}\" 创建数据文件夹失败：{error}",
                self.metadata.name
            );
        }
        path
    }

    /// 按名称异步获取玩家。
    ///
    /// # Arguments
    /// - `player_name`：要获取的玩家名称。
    ///
    /// # Returns
    /// 若找到则为玩家的可选引用，否则为 `None`。
    #[must_use]
    pub fn get_player_by_name(&self, player_name: &str) -> Option<Arc<Player>> {
        self.server.get_player_by_name(player_name)
    }

    /// 向插件上下文注册一个服务。
    ///
    /// 此方法允许你将服务实例与给定的名称关联，
    /// 使其可供插件或其他组件检索。
    /// 该服务必须包裹在 `Arc` 中并实现 `Payload`。
    ///
    /// # Arguments
    ///
    /// * `name` - 用于注册服务的唯一名称。
    /// * `service` - 要注册的服务实例，包装在 `Arc` 中。
    ///
    /// # Example
    ///
    /// ```ignore
    /// context.register_service("my_service", Arc::new(MyService::new())).await;
    /// ```
    pub async fn register_service<N: Into<String>, T: Payload + 'static>(
        &self,
        name: N,
        service: Arc<T>,
    ) {
        let name = name.into();
        // 同时发布到基于名称的跨插件注册表，以便 wasm
        // 插件可以发现（并通过 IPC 连接）此原生提供者。
        self.plugin_manager
            .register_service_provider(&name, &self.metadata.name, 0);
        let mut services = self.plugin_manager.services.write().await;
        if let Some((owner, _)) = services.get(&name)
            && owner != &self.metadata.name
        {
            tracing::warn!(
                "插件 \"{}\" 覆盖了此前由 \"{owner}\" 注册的服务 `{name}`",
                self.metadata.name
            );
        }
        services.insert(name, (self.metadata.name.clone(), service));
    }

    /// 移除此插件先前注册的一个服务。
    pub async fn unregister_service(&self, name: &str) {
        self.plugin_manager
            .unregister_service_provider(name, &self.metadata.name);
        let mut services = self.plugin_manager.services.write().await;
        // 只有拥有该服务的插件才能移除服务条目。
        if services
            .get(name)
            .is_some_and(|(owner, _)| owner == &self.metadata.name)
        {
            services.remove(name);
        }
    }

    /// 按名称和类型检索已注册的服务。
    ///
    /// 此方法尝试获取先前以给定名称注册的服务，
    /// 并使用基于名称的类型检查将其向下转型为所请求的类型。
    ///若服务存在且类型匹配，则返回 `Some(Arc<T>)`，否则返回 `None`。
    ///
    /// 此方法可安全地跨编译边界使用，因为它使用基于字符串的
    /// 类型识别，而不是 `TypeId`。
    ///
    /// # Arguments
    ///
    /// * `name` - 要获取的服务的名称。
    ///
    /// # Returns
    ///
    /// 一个 `Option<Arc<T>>`，若找到且类型匹配则包含该服务，否则为 `None`。
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(service) = context.get_service::<MyService>("my_service").await {
    ///     // Use the service
    /// }
    /// ```
    pub async fn get_service<T: Payload + 'static>(&self, name: &str) -> Option<Arc<T>> {
        let services = self.plugin_manager.services.read().await;
        let service = services.get(name)?.1.clone();
        <dyn Payload>::downcast_arc::<T>(service)
    }

    /// 规范化命令权限节点：裸名称会被加上此前缀
    /// 插件的命名空间；*其他*插件命名空间中的节点
    /// 被拒绝，与 [`Context::register_permission`] 的命名空间规则一致。
    pub(crate) fn normalize_command_permission(&self, permission: String) -> Option<String> {
        if !permission.contains(':') {
            return Some(format!("{}:{permission}", self.metadata.name));
        }
        if permission.starts_with(&format!("{}:", self.metadata.name)) {
            return Some(permission);
        }
        tracing::error!(
            plugin = %self.metadata.name,
            %permission,
            "插件试图注册一个基于外部权限节点的命令；已拒绝注册",
        );
        None
    }

    /// 以指定的权限等级向服务器注册一个新命令。
    ///
    /// 当命令标签已被其他插件占有时，该标签
    /// 而是注册为 `<plugin>:<label>`（Bukkit 的回退前缀），
    /// 因此原命令保留其裸标签。
    ///
    /// # Arguments
    /// - `node`：要注册的命令节点。
    /// - `permission`：执行该命令所需的权限等级。
    pub fn register_command<P: Into<String>>(
        &self,
        node: impl Into<crate::command::node::detached::CommandDetachedNode>,
        permission: P,
    ) {
        let Some(full_permission_node) = self.normalize_command_permission(permission.into())
        else {
            return;
        };

        let mut node = node.into();
        self.apply_command_fallback_prefix(&mut node);
        node.meta.source = Some(self.metadata.name.clone());
        node.owned
            .requirements
            .0
            .push(crate::command::node::Requirement(Arc::new(move |source| {
                source.has_permission(&full_permission_node)
            })));

        self.server.command_dispatcher.rcu(|dispatcher| {
            let mut new_dispatcher = (**dispatcher).clone();
            new_dispatcher.register(node.clone());
            Arc::new(new_dispatcher)
        });

        // 通知插件有命令已被注册。标签反映了
        // 上方应用的任何回退前缀。
        let mut event =
            crate::plugin::api::events::server::command_registered::CommandRegisteredEvent::new(
                node.meta.literal.to_string(),
                self.metadata.name.clone(),
            );
        self.server
            .plugin_manager
            .fire_blocking(&self.server, &mut event);

        self.reload_commands_for_everyone();
    }

    /// 将 `node` 的字面量重命名为 `<plugin>:<label>`，当裸标签已被
    /// 已被另一个插件占用（Bukkit 的回退前缀）。
    fn apply_command_fallback_prefix(
        &self,
        node: &mut crate::command::node::detached::CommandDetachedNode,
    ) {
        let normalized = node.meta.literal.to_ascii_lowercase();
        let conflict = {
            let dispatcher = self.server.command_dispatcher.load();
            dispatcher
                .get_command_source(&normalized)
                .is_some_and(|source| source != self.metadata.name)
        };
        if conflict {
            let fallback = format!("{}:{normalized}", self.metadata.name);
            tracing::info!(
                "Command /{normalized} is already registered by another plugin; \
                 registering /{fallback} as fallback"
            );
            node.meta.literal = fallback.into();
            node.meta.literal_lowercase = node.meta.literal.to_ascii_lowercase();
        }
    }

    /// 以指定的权限等级向服务器注册一个带别名的新命令。
    ///
    /// 冲突的标签会回退到 `<plugin>:<label>`，例如
    /// [`Context::register_command`]。
    pub fn register_command_with_aliases<P: Into<String>>(
        &self,
        node: impl Into<crate::command::node::detached::CommandDetachedNode>,
        aliases: &[String],
        permission: P,
    ) {
        let Some(full_permission_node) = self.normalize_command_permission(permission.into())
        else {
            return;
        };

        let mut node = node.into();
        self.apply_command_fallback_prefix(&mut node);
        node.meta.source = Some(self.metadata.name.clone());
        node.owned
            .requirements
            .0
            .push(crate::command::node::Requirement(Arc::new(move |source| {
                source.has_permission(&full_permission_node)
            })));

        self.server.command_dispatcher.rcu(|dispatcher| {
            let mut new_dispatcher = (**dispatcher).clone();
            new_dispatcher.register_with_aliases(node.clone(), aliases);
            Arc::new(new_dispatcher)
        });

        self.reload_commands_for_everyone();
    }

    /// 从服务器注销一个命令。
    ///
    /// # Arguments
    /// - `name`：要注销的命令名称。
    pub fn unregister_command(&self, name: &str) {
        self.server.command_dispatcher.rcu(|dispatcher| {
            let mut new_dispatcher = (**dispatcher).clone();
            new_dispatcher.deactivate_plugin_command_and_aliases(name);
            Arc::new(new_dispatcher)
        });

        self.reload_commands_for_everyone();
    }

    pub(crate) fn unregister_commands(&self) {
        let source = self.metadata.name.clone();
        self.server.command_dispatcher.rcu(|dispatcher| {
            let mut new_dispatcher = (**dispatcher).clone();
            new_dispatcher.deactivate_commands_from_source(&source);
            Arc::new(new_dispatcher)
        });

        self.reload_commands_for_everyone();
    }

    /// 为所有当前在线玩家重新加载（重发）所有命令。
    pub fn reload_commands_for_everyone(&self) {
        for world in self.server.worlds.load().iter() {
            for player in world.players.load().iter() {
                self.reload_commands_for(player);
            }
        }
    }

    /// 重新加载（重发）服务器上特定玩家的所有命令。
    ///
    /// # Arguments
    /// - `player`：要为其重新加载命令的玩家。
    pub fn reload_commands_for(&self, player: &Arc<Player>) {
        let command_dispatcher = self.server.command_dispatcher.load();
        client_suggestions::send_c_commands_packet(player, &self.server, &command_dispatcher);
    }

    /// 为此插件注册一个权限
    pub fn register_permission(&self, permission: Permission) -> Result<(), String> {
        // 确保权限具有正确的命名空间
        if !permission
            .node
            .starts_with(&format!("{}:", self.metadata.name))
        {
            return Err(format!(
                "权限 {} 必须使用插件的命名空间（{}）",
                permission.node, self.metadata.name
            ));
        }

        self.permission_manager.register_permission(permission)
    }

    /// 检查玩家是否拥有某项权限
    #[must_use]
    pub fn player_has_permission(&self, player_uuid: &uuid::Uuid, permission: &str) -> bool {
        // 如果玩家不在线，我们需要找到其管理员等级
        let player_op_level = self
            .server
            .get_player_by_uuid(*player_uuid)
            .map_or(PermissionLvl::Zero, |player| player.permission_lvl.load());

        self.permission_manager
            .has_permission(player_uuid, permission, player_op_level)
    }

    /// 为特定事件类型注册一个事件处理器。
    ///
    /// # Type Parameters
    /// - `E`：处理器将要响应的事件类型。
    /// - `H`：事件处理器的类型。
    ///
    /// # Arguments
    /// - `handler`：事件处理器的引用。
    /// - `priority`：事件处理器的优先级（`Lowest` 最先执行，
    ///   `Highest` 最后运行）。
    /// - `blocking`：指示处理器是否为阻塞式的布尔值。
    /// - `ignore_cancelled`：为 true 时，可取消事件将跳过处理器
    ///   已设置取消标志的事件（Bukkit 的 `ignoreCancelled`）。
    ///
    /// # Constraints
    /// 处理器必须实现 `EventHandler<E>` trait。
    pub fn register_event<E: Payload + 'static, H>(
        &self,
        handler: Arc<H>,
        priority: EventPriority,
        blocking: bool,
        ignore_cancelled: bool,
    ) where
        H: EventHandler<E> + 'static,
    {
        let typed_handler = Arc::new(TypedEventHandler {
            handler,
            priority,
            blocking,
            ignore_cancelled,
            source: Some(self.metadata.name.clone()),
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

    /// 注册可加载其他插件类型的自定义插件加载器。
    ///
    /// 此方法允许插件扩展服务器，使其支持加载
    /// 不同格式的插件（如 Lua、JavaScript、Python）。当新的
    /// 加载器已注册，插件管理器将自动尝试加载
    /// 用这个新加载器处理插件目录中任何先前无法加载的文件。
    ///
    /// # Arguments
    /// - `loader`：要注册的自定义插件加载器实现。
    ///
    /// # Returns
    /// 如果注册此加载器使得新插件被加载则为 `true`，否则为 `false`。
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create and register a custom Lua plugin loader
    /// let lua_loader = Arc::new(LuaPluginLoader::new());
    /// context.register_plugin_loader(lua_loader).await;
    /// ```
    pub async fn register_plugin_loader(
        &self,
        loader: Arc<dyn crate::plugin::loader::PluginLoader>,
    ) -> bool {
        let before_count = self.plugin_manager.loaded_plugins().len();
        self.plugin_manager.add_loader(&self.server, loader).await;
        let after_count = self.plugin_manager.loaded_plugins().len();

        // 如果加载了任何新插件则返回 true
        after_count > before_count
    }

    /// 通过 tracing crate 为插件初始化日志。
    pub fn init_log(&self) {
        if let Some(Some((_logger_impl, level, config))) = self.logger.get() {
            let fmt_layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(config.color)
                .with_target(true)
                .with_thread_names(config.threads)
                .with_thread_ids(config.threads);

            // 是 `try_init` 而非 `init`：第二个原生插件调用它时必须
            // 在已安装的全局订阅者上不应 panic。
            if config.timestamp {
                let fmt_layer = fmt_layer.with_timer(fmt::time::UtcTime::new(
                    time::macros::format_description!(
                        "[year]-[month]-[day] [hour]:[minute]:[second]"
                    ),
                ));
                let _ = tracing_subscriber::registry()
                    .with(*level)
                    .with(fmt_layer)
                    .try_init();
            } else {
                let fmt_layer = fmt_layer.without_time();
                let _ = tracing_subscriber::registry()
                    .with(*level)
                    .with(fmt_layer)
                    .try_init();
            }
        }
    }

    pub fn log(&self, message: impl std::fmt::Display) {
        let level = if let Some(Some((_, level, _))) = self.logger.get() {
            level.into_level().unwrap_or(Level::INFO)
        } else {
            Level::INFO
        };
        plugin_log!(level, &self.metadata.name, "{}", message);
    }
}
