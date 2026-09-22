use crate::block::registry::BlockRegistry;
use crate::command::commands::default_dispatcher;
use crate::command::commands::defaultgamemode::DefaultGamemode;
use crate::data::VanillaData;
use crate::data::player_server::ServerPlayerData;
use crate::entity::NBTStorage;
use crate::item::registry::ItemRegistry;
use crate::net::authentication::fetch_mojang_public_keys;
use crate::net::java::JavaClient;
use crate::net::{EncryptionError, GameProfile, PlayerConfig};
use crate::plugin::PluginManager;
use crate::plugin::player::player_login::PlayerLoginEvent;
use crate::plugin::server::server_broadcast::ServerBroadcastEvent;
use crate::server::tick_rate_manager::ServerTickRateManager;
use crate::world::WorldPortal;
use crate::world::custom_bossbar::CustomBossbars;
use crate::{
    command::node::dispatcher::CommandDispatcher, entity::player::Player, world::World,
    world::map::MapManager,
};
use arc_swap::ArcSwap;
use connection_cache::{CachedBranding, CachedStatus};
use key_store::KeyStore;
use papokin_config::{AdvancedConfiguration, BasicConfiguration, TelemetryConfig};
use papokin_data::dimension::Dimension;
use papokin_util::permission::PermissionManager;
use papokin_util::text::color::NamedColor;
use papokin_world::dimension::into_level;
use papokin_world::generation::generator::GeneratorInit;
use papokin_world::world::WorldPortalExt;
use tracing::{debug, error, info, warn};

use papokin_protocol::java::client::login::CEncryptionRequest;
use papokin_protocol::java::client::play::{CChangeDifficulty, CTabList};
use papokin_protocol::{ClientPacket, java::client::config::CPluginMessage};
use papokin_util::Difficulty;
use papokin_util::text::TextComponent;
use papokin_world::world_info::anvil::{
    AnvilLevelInfo, LEVEL_DAT_BACKUP_FILE_NAME, LEVEL_DAT_FILE_NAME,
};
use papokin_world::world_info::{LevelData, WorldInfoError, WorldInfoReader, WorldInfoWriter};
use rand::seq::IndexedRandom;
use rayon::prelude::*;
use rsa::RsaPublicKey;
use std::fs;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32};
use std::{future::Future, sync::atomic::Ordering, time::Duration};
use tokio::sync::OnceCell;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

pub mod connection_cache;
pub mod damage_type;
pub(crate) mod debug_profiler;
pub mod enchantment;
mod key_store;
pub mod permissions_file;
pub mod recipe;
pub mod registry;
pub mod scheduler;
pub mod seasonal_events;
pub mod server_test_manager;
pub mod tag;
pub mod tick_rate_manager;
pub mod ticker;

pub use recipe::RecipeManager;

use crate::data::advancement_data::AdvancementManager;
use crate::server::scheduler::TaskScheduler;

/// 表示一个 Minecraft 服务器实例。
pub struct Server {
    pub basic_config: BasicConfiguration,
    pub advanced_config: AdvancedConfiguration,
    pub telemetry_config: TelemetryConfig,

    pub data: VanillaData,

    /// 插件管理器
    pub plugin_manager: Arc<PluginManager>,

    /// 服务器的权限管理器。
    pub permission_manager: Arc<PermissionManager>,

    /// 管理用于安全通信的加密密钥。
    key_store: OnceCell<Arc<KeyStore>>,
    /// 管理服务器状态信息。
    listing: std::sync::Mutex<CachedStatus>,
    /// 保存服务器品牌信息。
    branding: CachedBranding,
    /// 保存并将命令分发给相应的处理器。
    pub command_dispatcher: ArcSwap<CommandDispatcher>,
    /// 方块行为。
    pub block_registry: Arc<BlockRegistry>,
    /// 物品行为。
    pub item_registry: Arc<ItemRegistry>,
    /// 管理服务器内的多个世界。
    pub worlds: ArcSwap<Vec<Arc<World>>>,
    /// 服务器上存在的所有维度。
    pub dimensions: Vec<Dimension>,
    /// 为容器分配唯一 ID。
    container_id: AtomicU32,
    pub recipe_manager: Arc<recipe::RecipeManager>,
    pub datapack_manager: Arc<crate::data::datapack::DatapackManager>,
    pub enchantment_manager: Arc<enchantment::EnchantmentManager>,
    /// 插件注册的自定义伤害类型及伤害类型名称解析。
    pub damage_type_manager: Arc<damage_type::DamageTypeManager>,
    /// 插件为同步注册表注册的自定义条目。
    pub registry_manager: Arc<registry::RegistryManager>,
    /// 插件对静态标签表的修改。
    pub tag_manager: Arc<tag::TagManager>,
    /// 为地图分配唯一 ID。
    map_id: AtomicI32,
    /// Mojang 的公钥，用于聊天会话签名
    /// 启动时从 Mojang API 拉取
    pub mojang_public_keys: ArcSwap<Vec<RsaPublicKey>>,
    /// 服务器的自定义 Boss 栏
    pub bossbars: std::sync::Mutex<CustomBossbars>,
    /// 管理服务器上的所有地图
    pub map_manager: MapManager,
    /// 玩家加入服务器时的默认游戏模式（每次重启时重置）
    pub defaultgamemode: std::sync::Mutex<DefaultGamemode>,
    /// 管理玩家数据存储
    pub player_data_storage: ServerPlayerData,
    /// 供 `/data` 命令使用的命令存储
    pub command_storage:
        std::sync::Mutex<std::collections::HashMap<String, papokin_nbt::compound::NbtCompound>>,
    /// 全局服务器秒表
    pub stopwatches: std::sync::Mutex<crate::world::stopwatches::Stopwatches>,
    /// 全局服务器随机序列
    pub random_sequences: std::sync::Mutex<crate::world::random_sequences::RandomSequences>,
    // 管理玩家进度
    pub advancement_manager: Arc<AdvancementManager>,
    // 服务器白名单是开启还是关闭
    pub white_list: AtomicBool,
    /// 管理服务器的刻速率、冻结与冲刺
    pub tick_rate_manager: Arc<ServerTickRateManager>,
    /// 存储最近 100 刻的耗时，用于性能分析
    pub tick_times_nanos: std::sync::Mutex<[i64; 100]>,
    /// 聚合的刻耗时，用于高效计算滚动平均值
    pub aggregated_tick_times_nanos: AtomicI64,
    /// 服务器已处理的刻总数
    pub tick_count: AtomicI32,
    /// 持有 `/debug` 使用的全服务器范围刻分析会话。
    pub(crate) debug_profiler: debug_profiler::DebugProfiler,
    /// 玩家闲置超时（分钟）（0 = 禁用）
    pub player_idle_timeout: AtomicI32,
    /// 管理已调度的任务（例如来自插件的）
    pub task_scheduler: Arc<TaskScheduler>,
    /// 管理已调度的数据包（资源）函数（`/schedule`）
    pub scheduled_functions: Arc<crate::server::scheduler::ScheduledFunctionQueue>,
    tasks: TaskTracker,
    pub runtime: tokio::runtime::Handle,

    // 与世界相关的内容，或许应该放进一个结构体
    pub level_info: Arc<ArcSwap<LevelData>>,
    world_info_writer: Arc<dyn WorldInfoWriter>,
}

impl Server {
    #[expect(clippy::too_many_lines)]
    #[must_use]
    pub async fn new(
        basic_config: BasicConfiguration,
        advanced_config: AdvancedConfiguration,
        telemetry_config: TelemetryConfig,
        vanilla_data: VanillaData,
    ) -> Arc<Self> {
        let permission_manager = Arc::new(PermissionManager::new());
        // 先注册默认命令，之后插件可以注册自己的命令
        let command_dispatcher = ArcSwap::from_pointee(default_dispatcher(
            &permission_manager,
            &advanced_config.commands,
        ));

        crate::command::set_broadcast_console_to_ops(
            advanced_config.commands.broadcast_console_to_ops,
        );

        let world_path = basic_config.get_world_path();

        let block_registry = super::block::registry::default_registry();

        let level_info = match AnvilLevelInfo.read_world_info(&world_path) {
            Ok(level_info) => {
                let dat_path = world_path.join(LEVEL_DAT_FILE_NAME);
                if dat_path.exists() {
                    let backup_path = world_path.join(LEVEL_DAT_BACKUP_FILE_NAME);
                    if let Err(err) = fs::copy(&dat_path, &backup_path) {
                        warn!("创建备份 {LEVEL_DAT_BACKUP_FILE_NAME} 失败：{err}");
                    }
                }
                level_info
            }
            Err(WorldInfoError::InfoNotFound) => {
                warn!(
                    "{} 中不存在 {LEVEL_DAT_FILE_NAME}，将使用种子 {} 创建新世界",
                    world_path.display(),
                    basic_config.seed.0 as i64
                );
                let overworld_gen = papokin_world::generation::generator::VanillaGenerator::new(
                    basic_config.seed,
                    Dimension::OVERWORLD,
                );
                let default_data =
                    LevelData::from_world_generator(basic_config.seed, &overworld_gen);
                if let Err(err) = AnvilLevelInfo.write_world_info(&default_data, &world_path) {
                    error!("保存 level.dat 失败：{err}");
                }
                default_data
            }
            Err(
                error @ (WorldInfoError::UnsupportedDataVersion(_)
                | WorldInfoError::UnsupportedLevelVersion(_)),
            ) => {
                error!("加载世界信息失败！");
                error!("{error}");
                error!("不支持的世界版本！更多信息请查看日志。");
                std::process::exit(1);
            }
            Err(error) => {
                error!("在 {} 中加载世界数据失败！", world_path.display());
                error!("{error}");
                error!(
                    "拒绝继续：默认世界将在现有区域文件之上生成不同的地形。请从 {LEVEL_DAT_BACKUP_FILE_NAME} 还原 {LEVEL_DAT_FILE_NAME}（其中还保存有世界种子的副本），或移开世界文件夹以创建新世界。"
                );
                error!("世界数据加载失败！更多信息请查看日志。");
                std::process::exit(1);
            }
        };

        let seed = level_info.world_gen_settings.seed;
        let level_info = Arc::new(ArcSwap::new(Arc::new(level_info)));

        let listing = std::sync::Mutex::new(CachedStatus::new(
            &basic_config,
            &advanced_config.networking.java.motd,
            advanced_config.networking.java.max_players,
        ));
        let defaultgamemode = std::sync::Mutex::new(DefaultGamemode {
            gamemode: basic_config.default_gamemode,
        });
        let players_dir = world_path.join("players");
        let player_data_storage = ServerPlayerData::new(
            players_dir.join("data"),
            Duration::from_secs(advanced_config.player_data.save_player_cron_interval),
            advanced_config.player_data.save_player_data,
        );
        let advancement_manager = Arc::new(AdvancementManager::new(
            players_dir.clone(),
            advanced_config.advancement.save_advancements,
        ));
        let white_list = AtomicBool::new(basic_config.white_list);

        let tick_rate_manager = Arc::new(ServerTickRateManager::new(basic_config.tps));

        let dimensions = {
            let mut dimensions = vec![Dimension::OVERWORLD];
            if basic_config.allow_nether {
                dimensions.push(Dimension::THE_NETHER);
            }
            if basic_config.allow_end {
                dimensions.push(Dimension::THE_END);
            }
            dimensions
        };
        info!(
            "已启用维度：{:?}",
            dimensions
                .iter()
                .map(|d| d.minecraft_name)
                .collect::<Vec<_>>()
        );

        let verify_plugin_signatures = advanced_config.plugins.verify_signatures;
        if !verify_plugin_signatures {
            warn!(
                "插件签名校验已禁用。仅当您完全信任您的插件及其来源时才应这样做，因为未签名或被篡改的 WASM 插件将被加载而不经过校验。"
            );
        }

        let registry_manager = Arc::new(registry::RegistryManager::new());
        let tag_manager = Arc::new(tag::TagManager::new(Arc::clone(&registry_manager)));

        let server = Self {
            basic_config,
            advanced_config,
            telemetry_config,
            data: vanilla_data,
            plugin_manager: Arc::new(PluginManager::new(verify_plugin_signatures)),
            permission_manager,
            container_id: 0.into(),
            recipe_manager: Arc::new(recipe::RecipeManager::new()),
            datapack_manager: Arc::new(crate::data::datapack::DatapackManager::new()),
            enchantment_manager: Arc::new(enchantment::EnchantmentManager::new()),
            damage_type_manager: Arc::new(damage_type::DamageTypeManager::new()),
            registry_manager,
            tag_manager,
            map_id: level_info.load().map_id.into(),
            worlds: ArcSwap::from_pointee(vec![]),
            dimensions,
            command_dispatcher,
            block_registry: block_registry.clone(),
            item_registry: super::item::items::default_registry(),
            key_store: OnceCell::new(),
            listing,
            branding: CachedBranding::new(),
            bossbars: std::sync::Mutex::new(CustomBossbars::new()),
            map_manager: MapManager::new(),
            defaultgamemode,
            player_data_storage,
            command_storage: std::sync::Mutex::new(std::collections::HashMap::new()),
            stopwatches: std::sync::Mutex::new(crate::world::stopwatches::Stopwatches::new()),
            random_sequences: std::sync::Mutex::new(
                crate::world::random_sequences::RandomSequences::new(),
            ),
            advancement_manager,
            white_list,
            tick_rate_manager,
            tick_times_nanos: std::sync::Mutex::new([0; 100]),
            aggregated_tick_times_nanos: AtomicI64::new(0),
            tick_count: AtomicI32::new(0),
            debug_profiler: debug_profiler::DebugProfiler::new(),
            tasks: TaskTracker::new(),
            runtime: tokio::runtime::Handle::current(),
            task_scheduler: Arc::new(TaskScheduler::new()),
            scheduled_functions: Arc::new(crate::server::scheduler::ScheduledFunctionQueue::new()),
            player_idle_timeout: AtomicI32::new(0),
            mojang_public_keys: ArcSwap::from_pointee(Vec::new()),
            world_info_writer: Arc::new(AnvilLevelInfo),
            level_info,
        };
        let server = Arc::new(server);

        // 加载服务器级权限声明（permissions.toml）（如果有）。
        if let Err(error) = permissions_file::load_permissions_file(
            &server.permission_manager,
            std::path::Path::new("permissions.toml"),
        ) {
            warn!("加载 permissions.toml 失败：{error}");
        }

        // 在后台任务中获取/生成密钥，以避免阻塞启动
        let server_clone = server.clone();
        server.spawn_task(async move {
            let key_store = Arc::new(KeyStore::new());
            let _ = server_clone.key_store.set(key_store);
        });

        if server.basic_config.allow_chat_reports {
            let server_clone = server.clone();
            server.spawn_task(async move {
                let auth_config = server_clone
                    .advanced_config
                    .networking
                    .java
                    .authentication
                    .clone();
                let keys = fetch_mojang_public_keys(&auth_config)
                    .await
                    .unwrap_or_else(|e| {
                        error!("获取 Mojang 密钥失败：{e}");
                        Vec::new()
                    });
                server_clone.mojang_public_keys.store(Arc::new(keys));
            });
        }

        // 引导阶段：声明 `load_order = startup` 的插件运行
        // 它们的 on_load/on_enable 会在世界创建之前调用，这样它们可以
        // 在世界加载之前注册数据或拦截生成过程。
        if server.advanced_config.plugins.enabled {
            match server
                .plugin_manager
                .load_plugins(&server, crate::plugin::api::LoadOrder::Startup)
                .await
            {
                Ok(duration) if !duration.is_zero() => {
                    info!("启动阶段插件已加载（等待 {}ms）", duration.as_millis());
                }
                Ok(_) => {}
                Err(err) => error!("{err}"),
            }
        }

        let mut worlds_vec = Vec::new();
        for dim in &server.dimensions {
            info!(
                "正在加载 {}",
                TextComponent::text(dim.minecraft_name.to_string())
                    .color_named(NamedColor::DarkGreen)
                    .to_pretty_console()
            );
            let config = Arc::new(server.advanced_config.world.clone());
            let level = into_level(dim.clone(), &config, world_path.clone(), seed);
            let world = Arc::new(World::load(
                level.clone(),
                server.level_info.clone(),
                dim.clone(),
                block_registry.clone(),
                Arc::downgrade(&server),
            ));
            let portal: Arc<dyn WorldPortalExt> = Arc::new(WorldPortal(world.clone()));
            level.world_portal.store(Arc::new(Some(portal)));
            worlds_vec.push(world);
        }

        for world in &worlds_vec {
            let mut world_init_event =
                crate::plugin::api::events::world::world_init::WorldInitEvent::new(world.clone());
            server
                .plugin_manager
                .fire(&server, &mut world_init_event)
                .await;

            let mut world_load_event =
                crate::plugin::api::events::world::world_load::WorldLoadEvent::new(world.clone());
            server
                .plugin_manager
                .fire(&server, &mut world_load_event)
                .await;
        }

        server.worlds.store(Arc::new(worlds_vec));

        info!("所有世界加载成功。");

        let enabled_packs = server.level_info.load().data_packs.enabled.clone();
        server
            .datapack_manager
            .load_all(&world_path, &enabled_packs, &server.recipe_manager);

        let source = crate::command::CommandSender::Console.into_source(&server);
        let _ = server
            .datapack_manager
            .execute_function(&server, &source, "#minecraft:load");

        server
    }

    /// 生成与此服务器关联的任务。使用此方法生成的所有任务都会被等待
    /// 在服务器关停时。这意味着任务应在合理（无循环）的时间内完成。
    pub fn spawn_task<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tasks.spawn_on(task, &self.runtime)
    }

    pub fn get_world_from_dimension(&self, dimension: &Dimension) -> Arc<World> {
        let worlds = self.worlds.load();
        worlds
            .iter()
            .find(|w| w.dimension.minecraft_name == dimension.minecraft_name)
            .cloned()
            .or_else(|| worlds.first().cloned())
            .unwrap_or_else(|| {
                error!("不存在默认世界");
                std::process::exit(1);
            })
    }

    pub fn create_world(self: &Arc<Self>, name: String, dimension: Dimension) -> Arc<World> {
        {
            let worlds = self.worlds.load();
            let world = worlds
                .iter()
                .find(|w| w.get_world_name() == name && w.dimension == dimension)
                .cloned();
            if let Some(world) = world {
                return world;
            }
        }

        let world_path = self.basic_config.get_world_path().join(name);
        let registry = self.block_registry.clone();
        let l_info = self.level_info.clone();
        let weak = Arc::downgrade(self);
        let config = Arc::new(self.advanced_config.world.clone());
        let seed = self.level_info.load().world_gen_settings.seed;

        let level =
            papokin_world::dimension::into_level(dimension.clone(), &config, world_path, seed);
        let world: World = World::load(level.clone(), l_info, dimension, registry, weak);
        let world = Arc::new(world);
        let portal: Arc<dyn WorldPortalExt> = Arc::new(WorldPortal(world.clone()));
        level.world_portal.store(Arc::new(Some(portal)));
        self.worlds.rcu(|worlds| {
            let mut new_worlds = (**worlds).clone();
            new_worlds.push(world.clone());
            new_worlds
        });
        let mut event =
            crate::plugin::api::events::world::world_init::WorldInitEvent::new(world.clone());
        self.plugin_manager.fire_blocking(self, &mut event);
        world
    }

    pub async fn unload_world(&self, name: &str) -> Result<(), String> {
        let worlds = self.worlds.load();
        let world_to_unload = worlds
            .iter()
            .find(|w| w.get_world_name() == name || w.dimension.minecraft_name == name)
            .cloned()
            .ok_or_else(|| format!("World '{name}' not found"))?;

        if let Some(first_world) = worlds.first()
            && Arc::ptr_eq(first_world, &world_to_unload)
        {
            return Err("Cannot unload the primary/default world".to_string());
        }

        let player_count = world_to_unload.players.load().len();
        if player_count > 0 {
            return Err(format!(
                "Cannot unload world '{name}': {player_count} players are still in this world"
            ));
        }

        world_to_unload.shutdown().await;
        world_to_unload.unload().await;

        self.worlds.rcu(|w_list| {
            let mut new_list = (**w_list).clone();
            new_list.retain(|w| !Arc::ptr_eq(w, &world_to_unload));
            new_list
        });

        Ok(())
    }

    pub fn save_world_info(&self) -> Result<(), WorldInfoError> {
        let level_data = self.level_info.load();
        self.world_info_writer
            .write_world_info(&level_data, &self.basic_config.get_world_path())
    }

    pub fn reload_datapacks(&self, server: &Arc<Self>) {
        let enabled_packs = self.level_info.load().data_packs.enabled.clone();
        let world_path = self.basic_config.get_world_path();
        self.datapack_manager
            .load_all(&world_path, &enabled_packs, &self.recipe_manager);

        let source = crate::command::CommandSender::Console.into_source(server);
        let _ = self
            .datapack_manager
            .execute_function(server, &source, "#minecraft:load");

        let dynamic_recipes = self.recipe_manager.get_dynamic_recipes_internal();
        for player in self.get_all_players() {
            let java_client = &player.client;
            let add_packet =
                papokin_protocol::java::client::play::CRecipeBookAdd::new(true, &dynamic_recipes);
            if let Ok(data) = java_client.serialize_packet(&add_packet) {
                java_client.try_enqueue_packet(data);
            }
        }

        // 通知插件服务器资源已重新加载。每个
        // 当前调用方通过命令或内部
        // 数据包操作，因此原因报告为 `command`。
        let mut event = crate::plugin::api::events::server::server_resources_reloaded::ServerResourcesReloadedEvent::new(
            "command".to_string(),
        );
        self.plugin_manager.fire_blocking(server, &mut event);
    }

    #[must_use]
    pub fn get_known_packs<'a>(
        &self,
        server_version: &'a str,
        loaded_packs: &'a [crate::data::datapack::LoadedDatapack],
    ) -> Vec<papokin_protocol::KnownPack<'a>> {
        self.datapack_manager
            .get_known_packs(self, server_version, loaded_packs)
    }

    #[must_use]
    pub fn get_enabled_features(&self) -> Vec<&'static str> {
        self.datapack_manager.get_enabled_features(self)
    }

    #[must_use]
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.datapack_manager.is_feature_enabled(self, feature)
    }

    pub async fn save_all(&self) -> Result<(), String> {
        if let Err(err) = self.save_world_info() {
            error!("保存世界信息失败：{err}");
            return Err(format!("Failed to save world info: {err}"));
        }

        if let Err(err) = self.player_data_storage.save_all_players(self) {
            error!("保存玩家数据失败：{err}");
            return Err(format!("Failed to save player data: {err}"));
        }

        if let Err(err) = self
            .advancement_manager
            .save_all_players(&self.get_all_players())
            .await
        {
            error!("保存玩家进度失败：{err}");
            return Err(format!("Failed to save player advancements: {err}"));
        }

        for world in self.worlds.load().iter() {
            world.save().await;
        }

        Ok(())
    }

    /// 向服务器添加一名新玩家。
    ///
    /// 此函数接收一个表示已连接客户端的 `Arc<Client>`，并执行以下操作：
    ///
    /// 1. 为玩家生成新的实体 ID。
    /// 2. 确定玩家的游戏模式（若配置中未指定，默认为生存模式）。
    /// 3. **(TODO: 从配置中选择默认值)** 为玩家选择世界（目前使用第一个世界）。
    /// 4. 使用提供的信息创建新的 `Player` 实例。
    /// 5. 将玩家加入所选的世界。
    /// 6. **(TODO: 若想提升在线人数可在此配置)** 可选地根据玩家的配置更新服务器列表信息。
    ///
    /// # Arguments
    ///
    /// * `client`: 表示已连接客户端的 `Arc<Client>`。
    ///
    /// # Returns
    ///
    /// 一个包含以下内容的元组：
    ///
    /// - `Arc<Player>`：指向新创建的玩家对象的引用。
    /// - `Arc<World>`：指向玩家被加入的世界的引用。
    ///
    /// # Note
    ///
    /// 你仍需将 `Player` 生成到某个 `World` 中，才能让其加入并变为可见。
    pub fn add_player(
        self: &Arc<Self>,
        client: Arc<JavaClient>,
        profile: GameProfile,
        config: Option<PlayerConfig>,
    ) -> Option<(Arc<Player>, Arc<World>)> {
        let gamemode = self
            .defaultgamemode
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .gamemode;

        let first_world = self.worlds.load().first().cloned()?;

        let (world, nbt) = if let Ok(Some(data)) = self.player_data_storage.load_data(&profile.id) {
            if let Some(dimension_key) = data.get_string("Dimension") {
                if let Some(dimension) = Dimension::from_name(dimension_key) {
                    let world = self.get_world_from_dimension(dimension);
                    (world, Some(data))
                } else {
                    warn!("玩家数据中的维度键无效：{dimension_key}");
                    (first_world, Some(data))
                }
            } else {
                // 玩家数据存在但没有 "Dimension" 键。
                (first_world, Some(data))
            }
        } else {
            // 未找到玩家数据或发生错误，默认使用主世界。
            (first_world, None)
        };

        let mut player = Player::new(
            client,
            profile,
            config.clone().unwrap_or_default(),
            &world,
            gamemode,
        );

        if let Some(mut nbt_data) = nbt {
            player.read_nbt(&mut nbt_data);
            // 数据文件本身就证明这是一位回归玩家。
            player.has_played_before.store(true, Ordering::Relaxed);
        }

        // 数据加载完成后用 Arc 包装
        let player = Arc::new(player);
        {
            let mut advancements = player
                .advancements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Err(e) = advancements.load() {
                warn!("加载玩家 {} 出错：{e}", player.gameprofile.id);
            }
            advancements.player = Arc::downgrade(&player);
        };

        send_cancellable_blocking! {{
            self;
            &mut PlayerLoginEvent::new(player.clone(), TextComponent::text("你已被踢出服务器"));
            'after: {
                player.screen_handler_sync_handler.store_player(player.clone());
                world.add_player(&player).is_ok().then(|| {
                    {
                        let mut user_cache = self
                            .data
                            .user_cache
                            .write()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        user_cache.upsert(player.gameprofile.id, player.gameprofile.name.clone());
                    };

                    // TODO: 若想让在线人数上升就做成配置
                    if let Some(config) = config {
                        // TODO: 做成配置，这样我们也可以直接忽略它，嘿嘿
                        if config.server_listing {
                            self.listing
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .add_player(&player);
                        }
                    }

                    (player, world)
                })
            }

            'cancelled: {
                player.kick(&event.kick_message);
                None
            }
        }}
    }

    pub fn remove_player(&self, player: &Player) {
        player.increment_stat(
            papokin_data::statistic::StatisticCategory::Custom,
            papokin_data::statistic::CustomStatistic::LeaveGame as i32,
            1,
        );
        // TODO: 若想让在线人数下降就做成配置
        self.listing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove_player(player);

        // 纯通知：在每个玩家拆除（登录后）时触发。
        if let Some(server_arc) = crate::net::server_arc(self) {
            let mut event = crate::plugin::api::events::player::player_connection_close::PlayerConnectionCloseEvent::new(
                player.gameprofile.id,
                player.gameprofile.name.clone(),
                player.client.address.to_string(),
            );
            server_arc
                .plugin_manager
                .fire_blocking(&server_arc, &mut event);
        }
    }

    pub async fn shutdown(&self) {
        self.tasks.close();
        debug!("等待服务器任务");
        self.tasks.wait().await;
        debug!("服务器任务等待完成");

        info!("正在启动世界");
        for world in self.worlds.load().iter() {
            world.shutdown().await;
        }
        let level_data = self.level_info.load();
        // 然后保存世界信息

        if let Err(err) = self
            .world_info_writer
            .write_world_info(&level_data, &self.basic_config.get_world_path())
        {
            error!("保存 level.dat 失败：{err}");
        }
        info!("世界已完成");
    }

    /// 向所有世界中的全部玩家广播数据包。
    ///
    /// 此函数向服务器管理的每个世界中所有已连接的玩家发送指定的数据包。
    ///
    /// # Arguments
    ///
    /// * `packet`: 要广播的数据包的引用。该数据包必须实现 `ClientPacket` trait。
    pub fn broadcast_packet_all<P: ClientPacket>(&self, packet: &P) {
        for world in self.worlds.load().iter() {
            world.broadcast_packet_all(packet);
        }
    }

    /// 向服务器上的所有玩家广播自定义服务器链接。
    pub fn broadcast_server_links(&self, links: &[papokin_protocol::Link<'_>]) {
        let packet = papokin_protocol::java::client::play::CPlayServerLinks::new(links);
        self.broadcast_packet_all(&packet);
    }

    pub fn broadcast_tab_list_header_footer(&self, header: &TextComponent, footer: &TextComponent) {
        let packet = CTabList::new(header, footer);
        for world in self.worlds.load().iter() {
            for player in world.players.load().iter() {
                *player
                    .tab_list_header
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = header.clone();
                *player
                    .tab_list_footer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = footer.clone();
            }
            world.broadcast_packet_all(&packet);
        }
    }

    pub fn broadcast_message(
        self: &Arc<Self>,
        message: &TextComponent,
        sender_name: &TextComponent,
        chat_type: u8,
        target_name: Option<&TextComponent>,
    ) {
        let mut event = ServerBroadcastEvent::new(message.clone(), sender_name.clone());
        self.plugin_manager.fire_blocking(self, &mut event);
        if !event.cancelled {
            for world in self.worlds.load().iter() {
                world.broadcast_message(&event.message, &event.sender, chat_type, target_name);
            }
        }
    }

    pub fn broadcast_chat_message(
        self: &Arc<Self>,
        message: &crate::net::chat::PlayerChatMessage,
        is_filtered: impl Fn(&crate::entity::player::Player) -> bool,
        sender_player: Option<&Arc<crate::entity::player::Player>>,
        chat_type: papokin_protocol::codec::var_int::VarInt,
        sender_name: &TextComponent,
        target_name: Option<&TextComponent>,
    ) {
        for world in self.worlds.load().iter() {
            world.broadcast_chat_message(
                message,
                &is_filtered,
                sender_player,
                chat_type,
                sender_name,
                target_name,
            );
        }
    }

    /// 获取服务器当前的难度。
    pub fn get_difficulty(&self) -> Difficulty {
        self.level_info.load().difficulty
    }

    /// 设置服务器的难度。
    ///
    /// 此函数更新服务器的难度等级，并将更改广播给所有玩家。
    /// 它还会遍历所有世界，以确保难度应用一致。
    /// 若 `force_update` 为 `Some(true)`，则无论当前状态如何都会设置难度。
    /// 若 `force_update` 为 `Some(false)` 或 `None`，则仅在难度未锁定时才会更新。
    ///
    /// # Arguments
    ///
    /// * `difficulty`: 要设置的新难度等级。应为 `Difficulty` 枚举的某个变体。
    /// * `force_update`: 可选的布尔值，若设为 `Some(true)`，则即使难度当前被锁定也强制更新。
    ///
    /// # Note
    ///
    /// 此函数不处理实际的生物生成选项更新，这是留待未来实现的 TODO 项。
    pub fn set_difficulty(&self, difficulty: Difficulty, force_update: bool) {
        let current_info = self.level_info.load();
        if current_info.difficulty_locked && !force_update {
            return;
        }

        let new_difficulty = if self.basic_config.hardcore {
            Difficulty::Hard
        } else {
            difficulty
        };

        let mut new_info = (**current_info).clone();

        new_info.difficulty = new_difficulty;
        let locked = new_info.difficulty_locked;
        self.level_info.store(Arc::new(new_info));

        for world in self.worlds.load().iter() {
            let old_difficulty = world.level_info.load().difficulty;
            world.set_difficulty(difficulty);
            world.broadcast_packet_all(&CChangeDifficulty::new(difficulty as u8, locked));

            // 通知插件每个世界的难度变更。纯
            // 通知：新难度已经生效。
            // `set_difficulty` 只有 `&self`，因此需要恢复出一个 `Arc<Server>`
            // 通过各世界的弱反向引用。
            if let Some(server) = crate::net::server_arc(self) {
                let mut event = crate::plugin::api::events::world::world_difficulty_change::WorldDifficultyChangeEvent::new(
                    world.clone(),
                    old_difficulty.name().to_string(),
                    difficulty.name().to_string(),
                );
                self.plugin_manager.fire_blocking(&server, &mut event);
            }
        }
    }

    /// 设置服务器的难度锁定状态，并向所有玩家广播该更新。
    pub fn set_difficulty_locked(&self, locked: bool) {
        let current_info = self.level_info.load();
        let mut new_info = (**current_info).clone();
        new_info.difficulty_locked = locked;
        let difficulty = new_info.difficulty;
        self.level_info.store(Arc::new(new_info));

        for world in self.worlds.load().iter() {
            world.broadcast_packet_all(&CChangeDifficulty::new(difficulty as u8, locked));
        }
    }

    /// 在所有世界中按用户名搜索玩家。
    ///
    /// 此函数遍历服务器管理的每个世界，尝试查找具有指定用户名的玩家。
    /// 若在任意世界中找到该玩家，则返回该玩家的 `Arc<Player>` 引用；否则返回 `None`。
    ///
    /// # Arguments
    ///
    /// * `name`: 要查找的玩家的用户名。
    ///
    /// # Returns
    ///
    /// 一个 `Option<Arc<Player>>`，若找到则包含该玩家，否则为 `None`。
    pub fn get_player_by_name(&self, name: &str) -> Option<Arc<Player>> {
        for world in self.worlds.load().iter() {
            if let Some(player) = world.get_player_by_name(name) {
                return Some(player);
            }
        }
        None
    }

    pub fn get_players_by_ip(&self, ip: IpAddr) -> Vec<Arc<Player>> {
        let mut players = Vec::<Arc<Player>>::new();

        for world in self.worlds.load().iter() {
            for player in world.players.load().iter() {
                if player.client.address.ip() == ip {
                    players.push(player.clone());
                }
            }
        }

        players
    }

    ///返回所有世界中的全部玩家。
    pub fn get_all_players(&self) -> Vec<Arc<Player>> {
        let mut players = Vec::<Arc<Player>>::new();

        for world in self.worlds.load().iter() {
            players.extend(world.players.load().iter().cloned());
        }

        players
    }

    pub fn for_each_player<F>(&self, mut f: F)
    where
        F: FnMut(&Arc<Player>),
    {
        let worlds = self.worlds.load();

        for world in worlds.iter() {
            let players = world.players.load();
            for player in players.iter() {
                f(player);
            }
        }
    }

    ///从任意一个世界返回随机玩家；若所有世界都为空，则返回 `None`。
    pub fn get_random_player(&self) -> Option<Arc<Player>> {
        let players = self.get_all_players();
        players.choose(&mut rand::rng()).map(Arc::<_>::clone)
    }

    /// 在所有世界中按 UUID 搜索玩家。
    ///
    /// 此函数遍历服务器管理的每个世界，尝试查找具有指定 UUID 的玩家。
    /// 若在任意世界中找到该玩家，则返回该玩家的 `Arc<Player>` 引用；否则返回 `None`。
    ///
    /// # Arguments
    ///
    /// * `id`: 要查找的玩家的 UUID。
    ///
    /// # Returns
    ///
    /// 一个 `Option<Arc<Player>>`，若找到则包含该玩家，否则为 `None`。
    pub fn get_player_by_uuid(&self, id: uuid::Uuid) -> Option<Arc<Player>> {
        for world in self.worlds.load().iter() {
            if let Some(player) = world.get_player_by_uuid(id) {
                return Some(player);
            }
        }
        None
    }

    /// 统计所有世界中的玩家总数。
    ///
    /// 此函数遍历每个世界，并汇总当前连接到该世界的玩家数量。
    ///
    /// # Returns
    ///
    /// 连接到服务器的玩家总数。
    pub fn get_player_count(&self) -> usize {
        let mut count = 0;
        for world in self.worlds.load().iter() {
            count += world.players.load().len();
        }
        count
    }

    /// 类似于 [`Server::get_player_count`] >= n，但可能更高效，因为一旦找到 n 个玩家就会停止遍历所有世界。
    pub fn has_n_players(&self, n: usize) -> bool {
        let mut count = 0;
        for world in self.worlds.load().iter() {
            count += world.players.load().len();
            if count >= n {
                return true;
            }
        }
        false
    }

    /// 返回服务器允许的最大玩家数量。
    #[must_use]
    pub const fn max_players(&self) -> u32 {
        self.advanced_config.networking.java.max_players
    }

    /// 若配置中已启用，则启动后台遥测任务。
    pub fn start_telemetry(self: &Arc<Self>) {
        crate::telemetry::start_telemetry(self.clone());
    }

    /// 生成新的容器 ID。
    pub fn new_container_id(&self) -> u32 {
        self.container_id.fetch_add(1, Ordering::SeqCst)
    }

    /// 生成新的地图 ID。
    pub fn next_map_id(&self) -> i32 {
        let id = self.map_id.fetch_add(1, Ordering::SeqCst);
        self.level_info.rcu(|level_info| {
            let mut new_level_info = (**level_info).clone();
            new_level_info.map_id = self.map_id.load(Ordering::SeqCst);
            new_level_info
        });
        id
    }

    pub const fn get_branding(&self) -> CPluginMessage<'_> {
        self.branding.get_branding()
    }

    pub const fn get_status(&self) -> &std::sync::Mutex<CachedStatus> {
        &self.listing
    }

    async fn get_or_init_key_store(&self) -> &Arc<KeyStore> {
        self.key_store
            .get_or_init(|| async {
                let (tx, rx) = tokio::sync::oneshot::channel();
                rayon::spawn(move || {
                    let _ = tx.send(Arc::new(KeyStore::new()));
                });
                rx.await.unwrap_or_else(|_| Arc::new(KeyStore::new()))
            })
            .await
    }

    pub async fn encryption_request<'a>(
        &'a self,
        verification_token: &'a [u8; 4],
        should_authenticate: bool,
    ) -> CEncryptionRequest<'a> {
        self.get_or_init_key_store().await.encryption_request(
            "",
            verification_token,
            should_authenticate,
        )
    }

    pub async fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let key_store = self.get_or_init_key_store().await.clone();
        let data = data.to_vec();
        let (tx, rx) = tokio::sync::oneshot::channel();
        rayon::spawn(move || {
            let _ = tx.send(key_store.decrypt(&data));
        });
        rx.await.map_err(|_| EncryptionError::FailedDecrypt)?
    }

    pub fn digest_secret(&self, secret: &[u8]) -> String {
        self.key_store.get().map_or_else(
            || KeyStore::new().get_digest(secret),
            |key_store| key_store.get_digest(secret),
        )
    }

    /// 服务器主刻方法。现在同时处理玩家/网络的刻推进（始终运行）与其他更新
    /// 以及世界/游戏逻辑的逐刻运行（受冻结状态影响）。
    pub fn tick(self: &Arc<Self>) {
        if self.tick_rate_manager.runs_normally() || self.tick_rate_manager.is_sprinting() {
            self.tick_worlds();
            // 即使游戏冻结，也始终运行玩家与网络的刻逻辑
        } else {
            self.tick_players_and_network();
        }
    }

    /// 驱动即使在游戏冻结时也必须运行的关键服务器功能。
    /// 这包括玩家的刻处理（网络、保活包）以及将世界更新刷送给客户端。
    pub fn tick_players_and_network(self: &Arc<Self>) {
        let worlds = self.worlds.load();
        let handle = self.runtime.clone();

        for world in worlds.iter() {
            world.flush_block_updates();
            world.flush_synced_block_events();

            let players = world.players.load();
            let player_handle = handle.clone();
            players.par_iter().for_each(|player| {
                let _guard = player_handle.enter();
                player.tick(self);
            });
        }
    }

    /// 驱动所有世界的游戏逻辑。这是受 `/tick freeze` 影响的部分。
    pub fn tick_worlds(self: &Arc<Self>) {
        let source = crate::command::CommandSender::Console
            .into_source(self)
            .with_silent();
        let _ = self
            .datapack_manager
            .execute_function(self, &source, "#minecraft:tick");

        self.task_scheduler.tick(self);
        self.scheduled_functions.tick(
            self,
            self.tick_count.load(std::sync::atomic::Ordering::Relaxed) as u64,
        );

        let worlds = self.worlds.load();
        let handle = self.runtime.clone();

        worlds.par_iter().for_each(|world| {
            let _guard = handle.enter();
            world.tick(self);
        });

        // 全局任务
        self.player_data_storage.tick(self);
    }

    /// 用上一次刻的时长更新刻耗时统计。
    pub fn update_tick_times(&self, tick_duration_nanos: i64) {
        let tick_count = self.tick_count.fetch_add(1, Ordering::Relaxed);
        let index = (tick_count % 100) as usize;

        let mut tick_times = self
            .tick_times_nanos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let old_time = tick_times[index];
        tick_times[index] = tick_duration_nanos;
        drop(tick_times);

        self.aggregated_tick_times_nanos
            .fetch_add(tick_duration_nanos - old_time, Ordering::Relaxed);

        let target_tick_nanos = self.tick_rate_manager.nanoseconds_per_tick();
        let idle_nanos = target_tick_nanos.saturating_sub(tick_duration_nanos);

        let sample_slice = [
            tick_duration_nanos.max(target_tick_nanos),
            tick_duration_nanos,
            0,
            idle_nanos,
        ];
        let packet = papokin_protocol::java::client::play::CDebugSample::new(
            &sample_slice,
            papokin_protocol::codec::var_int::VarInt(0),
        );
        for world in self.worlds.load().iter() {
            let players = world.players.load();
            let recipients = players
                .iter()
                .filter(|player| {
                    player.subscribed_debug_sample.load(Ordering::Relaxed)
                        && player.permission_lvl.load() >= papokin_util::PermissionLvl::Two
                })
                .map(|player| player.client.as_ref());
            World::broadcast_java_clients(&packet, recipients);
        }
    }

    /// 获取最近 100 刻的滚动平均刻时间，单位为纳秒。
    pub fn get_average_tick_time_nanos(&self) -> i64 {
        let tick_count = self.tick_count.load(Ordering::Relaxed);
        let sample_size = (tick_count as usize).min(100);
        if sample_size == 0 {
            return 0;
        }
        self.aggregated_tick_times_nanos.load(Ordering::Relaxed) / sample_size as i64
    }

    ///返回每刻平均毫秒数（MSPT）。
    pub fn get_mspt(&self) -> f64 {
        let avg_nanos = self.get_average_tick_time_nanos();
        // 将纳秒转换为十进制毫秒
        avg_nanos as f64 / 1_000_000.0
    }

    ///返回每秒刻数（TPS）。
    pub fn get_tps(&self) -> f64 {
        let mspt = self.get_mspt();
        if mspt <= 0.0 {
            return 0.0;
        }
        1000.0 / mspt
    }

    ///返回最近 100 个刻耗时的副本。
    pub fn get_tick_times_nanos_copy(&self) -> [i64; 100] {
        *self
            .tick_times_nanos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub async fn execute_remote_command(self: &Arc<Self>, command: String) {
        let mut remote_event = crate::plugin::api::events::server::remote_server_command::RemoteServerCommandEvent::new(
            command,
        );
        self.plugin_manager.fire(self, &mut remote_event).await;
    }

    pub async fn register_service(self: &Arc<Self>, service_name: String) {
        let mut service_event =
            crate::plugin::api::events::server::service_register::ServiceRegisterEvent::new(
                service_name,
            );
        self.plugin_manager.fire(self, &mut service_event).await;
    }

    pub async fn unregister_service(self: &Arc<Self>, service_name: String) {
        let mut service_event =
            crate::plugin::api::events::server::service_unregister::ServiceUnregisterEvent::new(
                service_name,
            );
        self.plugin_manager.fire(self, &mut service_event).await;
    }

    pub async fn tab_complete(self: &Arc<Self>, buffer: String, completions: Vec<String>) {
        let mut tab_event = crate::plugin::api::events::server::tab_complete::TabCompleteEvent::new(
            buffer,
            completions,
        );
        self.plugin_manager.fire(self, &mut tab_event).await;
    }

    pub async fn enable_plugin(self: &Arc<Self>, plugin_name: String) {
        let mut enable_event =
            crate::plugin::api::events::server::plugin_enable::PluginEnableEvent::new(plugin_name);
        self.plugin_manager.fire(self, &mut enable_event).await;
    }

    pub async fn disable_plugin(self: &Arc<Self>, plugin_name: String) {
        let mut disable_event =
            crate::plugin::api::events::server::plugin_disable::PluginDisableEvent::new(
                plugin_name,
            );
        self.plugin_manager.fire(self, &mut disable_event).await;
    }
}
