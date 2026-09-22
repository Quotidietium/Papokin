//! Pumpkin 插件 API。
#![warn(missing_docs)]
#![allow(
    clippy::undocumented_unsafe_blocks,
    clippy::option_if_let_else,
    clippy::collection_is_never_read,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::panic
)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//!
//! 本 crate 提供编写一个编译
//! 为 WebAssembly 的 Pumpkin 服务器插件所需的一切。插件由一个实现 [`Plugin`] 的类型构成，通过
//! [`register_plugin!`] 宏注册。
//!
//! # Quick start
//!
//! ```rust,ignore
//! use papokin_plugin_api::{Plugin, PluginMetadata, Context, register_plugin, permissions::permissions};
//!
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn new() -> Self { MyPlugin }
//!     fn metadata(&self) -> PluginMetadata {
//!         PluginMetadata {
//!             name: "my-plugin".into(),
//!             version: "0.1.0".into(),
//!             authors: vec!["you".into()],
//!             description: "An example plugin.".into(),
//!             dependencies: vec![],
//!             permissions: vec![permissions::NETWORK_DNS.into()],
//!             load_after: vec!["some-lib".into()],
//!             load_before: vec![],
//!             provides: vec![],
//!             load_order: LoadOrder::PostWorld,
//!         }
//!     }
//! }
//!
//! register_plugin!(MyPlugin);
//! ```
//!
//! # Persisting data
//!
//! 插件以带 WASI 的 WebAssembly 运行，因此要保存跨
//! 服务器重启仍存在的数据，需用你
//! 所用语言的常规文件 API（如 Rust 的 `std::fs`）。没有独立的
//! 存储 API；文件系统即存储。
//!
//! 每个插件都有一个私有数据文件夹。使用方法：
//!
//! 1. 请求 `fs.read.data` 和/或 `fs.write.data` 权限
//!    （`permissions::FS_READ_DATA` / `permissions::FS_WRITE_DATA`）权限
//!    在你的 [`PluginMetadata`] 中。没有这些权限，该文件夹不可访问。
//! 2. 在 `on_load` 或 `on_unload` 内通过 context 的 `get_data_folder` 方法获取
//!    `on_load` 或 `on_unload` 内部。返回的路径是从
//!    WASI 沙箱内部看到的文件夹。
//! 3. 用常规的文件 API 在该路径下读写文件。
//!
//! ```rust,ignore
//! fn on_load(&self, context: &Context) -> Result<(), String> {
//!     let path = format!("{}/state.json", context.get_data_folder());
//!     let saved = std::fs::read_to_string(&path).unwrap_or_default();
//!     // ...parse and use `saved`, then later write it back...
//!     Ok(())
//! }
//! ```

use crate::{
    commands::{COMMAND_HANDLERS, COMMAND_SUGGESTION_HANDLERS},
    events::EVENT_HANDLERS,
    logging::WitSubscriber,
    scheduler::TASK_HANDLERS,
    text::TextComponent,
};
use std::sync::OnceLock;

/// 方块定义与方块类型辅助。
pub mod block;
/// 区块快照与区块加载控制。
pub mod chunk;
/// 生物实体与玩家的只读战斗追踪查询。
pub mod combat;
/// 插件命令注册与处理工具。
pub mod commands;
/// 插件配置文件（等同于 getConfig）。
pub mod config;
/// 客户端 cookie 存储（等同于 Paper 的 ClientCookie API）。
pub mod cookie;
/// 自定义伤害类型注册与构建器工具。
pub mod damage_type;
/// 数据包管理与查询工具。
pub mod datapack;
/// 展示与交互实体的工具与构建器。
pub mod display;
/// 末影龙之战（DragonBattle）的查看与控制。
pub mod dragon;
/// 自定义附魔注册与构建器工具。
pub mod enchantment;
/// 事件系统与事件处理器。
pub mod events;
mod ext;
pub(crate) mod generated;
/// 统一的物品栏与容器管理工具。
pub mod inventory;
/// 带类型的物品定义与 `ItemStack` 构造辅助函数。
pub mod item;
/// 战利品表查询与生成。
pub mod loot;
/// 自定义地图渲染（MapView）：像素绘制、光标、地形渲染。
pub mod map;
/// 商人（村民/流浪商人）交易报价管理。
pub mod merchant;
/// 插件消息通道（等同于 Messenger）。
pub mod messaging;
/// 专门的生物实体包装器与辅助工具。
pub mod mobs;
/// 插件权限常量。
///
/// 在 `PluginMetadata` 中使用这些内容，以请求访问特定的宿主功能。
pub mod permissions;
/// 自定义配方注册与构建器工具。
pub mod recipe;
/// 自定义注册表条目注册与查询工具。
pub mod registry;
/// 调度器工具。
pub mod scheduler;
/// 服务器级查询：构建信息与离线玩家查找。
pub mod server;
/// 跨插件服务注册表（等同于 ServicesManager）。
pub mod services;
/// 结构模板的注册与放置。
pub mod structure;
/// 标签修改与查询工具。
pub mod tag;
/// 记分板队伍管理与构建器工具。
pub mod team;
/// 自定义世界与区块生成工具及 trait。
pub mod worldgen;

/// 命令 WIT API 再导出。
pub mod command {
    pub use crate::wit::papokin::plugin::command::{
        Arg, ArgumentType, Command, CommandError, CommandNode, CommandSender, CommandSuggestion,
        CommandSuggestions, ConsumedArgs, StringType, SuggestionRequest,
    };
}

pub use wit::papokin::plugin::{
    advancement as advancement_wit, block_entity, boss_bar, combat as combat_wit,
    command as command_wit, common,
    context::{self, Context, MarketplaceMetadata, Server},
    damage_types as damage_types_wit, data_components, datapack as datapack_wit,
    display as display_wit, dragon as dragon_wit, enchantments as enchantments_wit, entity,
    entity_statuses as entity_statuses_wit,
    entity_types::EntityType,
    event::{self as events_wit, EventType},
    game_events as game_events_wit, gui, i18n, inventory as inventory_wit, ipc, item_stack,
    java_dialogs, java_packets, merchant as merchant_wit, particles, permission, player,
    potions as potions_wit, recipe as recipe_wit, registry as registry_wit, scoreboard,
    screens as screens_wit, statistics as statistics_wit, tag as tag_wit, text, uuid, world,
};

// 常用插件类型的便捷再导出，插件开发者可以
// 直接按名使用它们（例如为 GUI 或 `/give` 构建一个 `ItemStack`）。
pub use block::{BlockStateTypeExt, BlockType, BlockTypeExt, IntoBlockKey};
pub use combat::{CombatEntry, PlayerCombatExt};
pub use cookie::PlayerCookieExt;
pub use damage_type::{
    CustomDamageType, DamageEffects, DamageScaling, DamageTypeBuilder, DamageTypeError,
    DamageTypeManager, DeathMessageType, RegistrableDamageType,
};
pub use damage_types_wit::DamageType;
pub use datapack::{DatapackInfo, DatapackManager, EnablePosition};
pub use display::{
    BillboardMode, BlockDisplayEntity, DisplayEntity, DisplayEntityExt, DisplayTransformation,
    EntityDisplayExt, InteractionEntity, ItemDisplayEntity, ItemDisplayEntityExt, ItemDisplayMode,
    Quaternionf, TextAlignment, TextDisplayEntity, TextDisplayEntityExt, TransformationBuilder,
    Vector3f,
};
pub use enchantment::{
    AttributeModifierSlot, CustomEnchantment, CustomEnchantmentValue, Enchantment,
    EnchantmentBuilder, EnchantmentError, EnchantmentManager, RegistrableEnchantment,
};
pub use entity_statuses_wit::EntityStatus;
pub use events::{EventHandler, FromIntoEvent};
pub use ext::player::{PlayerCooldownExt, PlayerEnderChestExt};
pub use game_events_wit::GameEvent;
pub use inventory::{Inventory, PlayerInventory};
pub use item::{IntoItemKey, Item, ItemStackExt};
pub use merchant::{EntityMerchantExt, Merchant, TradeOffer, TradeOfferBuilder};
pub use mobs::{
    Ageable, AgeableData, BrainMemory, Cat, CatData, Creeper, CreeperData, DyeColor, Enderman,
    EndermanData, EntityCastExt, Fox, FoxData, IronGolem, IronGolemData, MemoryStatus, MobCast,
    MobData, Sheep, SheepData, Shulker, ShulkerData, Slime, SlimeData, Villager, VillagerData,
    VillagerProfession, Wolf, WolfData, Zombie, ZombieData,
};
pub use potions_wit::PotionType;
pub use recipe::{
    BrewingRecipeBuilder, CookingRecipeBuilder, Ingredient, RecipeCategory, RecipeError,
    RecipeManager, RegistrableRecipe, ShapedRecipeBuilder, ShapelessRecipeBuilder,
    SmithingTransformRecipeBuilder, SmithingTrimRecipeBuilder, StonecuttingRecipeBuilder,
};
pub use registry::{RegistryError, RegistryManager};
pub use screens_wit::Screen;
pub use server::{BuildInfo, OfflinePlayerInfo};
pub use statistics_wit::{CustomStatistic, StatisticCategory};
pub use tag::{TagError, TagManager};
pub use team::{PlayerTeamExt, ScoreboardTeamExt, Team, TeamSettingsBuilder};
pub use wit::papokin::plugin::attributes::{Attribute, AttributeModifier, ModifierOperation};
pub use wit::papokin::plugin::item_stack::{ItemAttributeModifier, ItemStack};
pub use wit::papokin::plugin::player::Player;
pub use wit::papokin::plugin::scoreboard::{CollisionRule, NametagVisibility, TeamSettings};
pub use wit::papokin::plugin::server::Dimension;
pub use wit::papokin::plugin::world::{
    Block, BlockDirection, BlockState, BlockStateInfo, Chunk, ChunkSnapshot, Entity, Flammable,
    LivingEntity, Mob, PathNodeType, RayTraceBlockResult, RayTraceEntityResult, RaycastResult,
    SpawnCategory, TeleportFlags, World, WorldBorder,
};
pub use worldgen::{ChunkBuffer, ChunkGenerator, GenerationPhase, GeneratorManager};

/// 进度 WIT API 再导出。
pub mod advancement {
    pub use crate::wit::papokin::plugin::advancement::{
        AdvancementDisplay, AdvancementInfo, AdvancementProgress, FrameType,
    };
}

/// Java 版对话框 WIT API 再导出。
pub mod java_dialog {
    pub use crate::wit::papokin::plugin::java_dialogs::{
        Action, ActionButton, AfterAction, CustomClickAction, Dialog, DialogBody, DialogInput,
        DialogInputBool, DialogInputNumberRange, DialogInputSingleOption, DialogInputText,
        DialogType, Link, LinkLabel, LinkType,
    };
}

/// 基于 WIT 的日志订阅器。
pub mod logging;

#[allow(clippy::too_many_arguments, missing_docs)]
mod wit {
    wit_bindgen::generate!({
        skip: ["init-plugin"],
        path: "../papokin-plugin-wit/v0.1",
        world: "plugin",
        chainable_methods: [
            "papokin:plugin/command@0.1.0#command",
            "papokin:plugin/command@0.1.0#command-node",
            "papokin:plugin/text@0.1.0#text-component"
        ]
    });

    use super::Component;
    export!(Component);
}

struct Component;

/// 向服务器描述插件的元数据。
pub struct PluginMetadata {
    /// 插件的人类可读名称。
    pub name: String,
    /// 插件的版本字符串（例如 `"1.0.0"`）。
    pub version: String,
    /// 插件作者列表。
    pub authors: Vec<String>,
    /// 插件功能的简短描述。
    pub description: String,
    /// 硬依赖：这些插件中任意一个缺失时本插件加载失败。
    /// 缺失。可以引用在其他插件中声明的能力名称
    /// `provides`。
    pub dependencies: Vec<String>,
    /// 插件请求的权限列表。
    pub permissions: Vec<String>,
    /// 软排序边：当指定的插件存在时，将本插件排在这些插件之后加载
    /// 存在时才会包含。缺失的名称会被忽略。
    pub load_after: Vec<String>,
    /// 软排序边：当指定的插件存在时，将本插件排在这些插件之前加载
    /// 存在时才会包含。缺失的名称会被忽略。
    pub load_before: Vec<String>,
    /// 本插件满足的、供其他插件声明依赖的
    /// 边缘。
    pub provides: Vec<String>,
    /// 此插件加载时所处的启动阶段。
    pub load_order: LoadOrder,
}

/// 相对于服务器启动，插件应在何时加载。
pub use wit::exports::papokin::plugin::metadata::LoadOrder;

impl wit::exports::papokin::plugin::metadata::Guest for Component {
    /// 向宿主返回插件元数据。
    fn get_metadata() -> wit::exports::papokin::plugin::metadata::PluginMetadata {
        let metadata = plugin().metadata();
        wit::exports::papokin::plugin::metadata::PluginMetadata {
            name: metadata.name,
            version: metadata.version,
            authors: metadata.authors,
            description: metadata.description,
            dependencies: metadata.dependencies,
            permissions: metadata.permissions,
            load_after: metadata.load_after,
            load_before: metadata.load_before,
            provides: metadata.provides,
            load_order: metadata.load_order,
        }
    }
}

impl wit::Guest for Component {
    /// WIT 入口点——委托给 [`Plugin::on_load`]。
    fn on_load(context: Context) -> Result<(), String> {
        plugin().on_load(context)
    }

    /// WIT 入口点——委托给 [`Plugin::on_enable`]。
    fn on_enable(context: Context) -> Result<(), String> {
        plugin().on_enable(context)
    }

    /// WIT 入口点——委托给 [`Plugin::on_disable`]。
    fn on_disable(context: Context) -> Result<(), String> {
        plugin().on_disable(context)
    }

    /// WIT 入口点——委托给 [`Plugin::on_unload`]。
    fn on_unload(context: Context) -> Result<(), String> {
        plugin().on_unload(context)
    }

    /// WIT 入口点——将传入的事件分发给 `event_id` 对应的已注册处理器。
    ///
    /// 如果给定 ID 没有注册处理器，则原样返回该事件。
    fn handle_event(event_id: u32, server: Server, event: events::Event) -> events::Event {
        let handler = EVENT_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&event_id)
            .cloned();
        if let Some(handler) = handler {
            handler.handle_erased(server, event)
        } else {
            event
        }
    }

    /// WIT 入口点——将传入的命令调用分发给 `command_id` 对应的已注册处理器。
    ///
    ///若给定 ID 没有注册任何处理器，则返回 [`CommandError`](command::CommandError)。
    fn handle_command(
        command_id: u32,
        sender: command::CommandSender,
        server: Server,
        args: command::ConsumedArgs,
    ) -> Result<i32, command::CommandError> {
        let handler = COMMAND_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&command_id)
            .cloned();
        handler.map_or_else(
            || {
                Err(command::CommandError::CommandFailed(TextComponent::text(
                    &format!("未为命令 id {command_id} 注册处理器"),
                )))
            },
            |handler| handler.handle(sender, server, args),
        )
    }

    /// WIT 入口点——将传入的命令建议请求分发给已注册的处理器。
    fn handle_command_suggestion(
        handler_id: u32,
        sender: command::CommandSender,
        server: Server,
        request: command::SuggestionRequest,
    ) -> command::CommandSuggestions {
        let handler = COMMAND_SUGGESTION_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&handler_id)
            .cloned();
        if let Some(handler) = handler {
            handler.suggest(sender, server, request)
        } else {
            command::CommandSuggestions {
                start: request.start,
                length: 0,
                values: Vec::new(),
            }
        }
    }

    /// WIT 入口点——将计划任务调用分发给 `handler_id` 对应的已注册处理器。
    fn handle_task(handler_id: u32, server: Server) {
        let handler = TASK_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_for_invocation(handler_id);
        if let Some(handler) = handler {
            handler(server);
        }
    }

    fn handle_ai_goal_can_start(goal_id: u32, server: Server, entity: entity::Entity) -> bool {
        let goal = crate::ai::AI_GOAL_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(goal_id);
        if let Some(goal) = goal {
            goal.can_start(server, entity)
        } else {
            false
        }
    }

    fn handle_ai_goal_should_continue(
        goal_id: u32,
        server: Server,
        entity: entity::Entity,
    ) -> bool {
        let goal = crate::ai::AI_GOAL_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(goal_id);
        if let Some(goal) = goal {
            goal.should_continue(server, entity)
        } else {
            false
        }
    }

    fn handle_ai_goal_start(goal_id: u32, server: Server, entity: entity::Entity) {
        let goal = crate::ai::AI_GOAL_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(goal_id);
        if let Some(goal) = goal {
            goal.start(server, entity);
        }
    }

    fn handle_ai_goal_tick(goal_id: u32, server: Server, entity: entity::Entity) {
        let goal = crate::ai::AI_GOAL_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(goal_id);
        if let Some(goal) = goal {
            goal.tick(server, entity);
        }
    }

    fn handle_ai_goal_stop(goal_id: u32, server: Server, entity: entity::Entity) {
        let goal = crate::ai::AI_GOAL_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(goal_id);
        if let Some(goal) = goal {
            goal.stop(server, entity);
        }
    }

    fn handle_ipc_message(
        sender: wit::PluginId,
        message: wit::IpcMessage,
    ) -> Result<wit::IpcMessage, String> {
        plugin().handle_ipc_message(sender, message)
    }

    /// WIT 入口点——在已注册通道上分发一条玩家插件消息。
    fn handle_plugin_message(player_uuid: String, channel: String, data: Vec<u8>) {
        plugin().on_plugin_message(&player_uuid, &channel, &data);
    }

    fn handle_generate_phase(
        generator_id: u32,
        phase: wit::papokin::plugin::world::GenerationPhase,
        chunk: wit::papokin::plugin::world::ChunkBuffer,
    ) {
        let generator = crate::worldgen::GENERATOR_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&generator_id)
            .cloned();
        if let Some(generator) = generator {
            let mut buffer = crate::worldgen::ChunkBuffer::new(chunk);
            match phase {
                wit::papokin::plugin::world::GenerationPhase::Biomes => {
                    generator.generate_biomes(&mut buffer);
                }
                wit::papokin::plugin::world::GenerationPhase::Noise => {
                    generator.generate_noise(&mut buffer);
                }
                wit::papokin::plugin::world::GenerationPhase::Surface => {
                    generator.generate_surface(&mut buffer);
                }
                wit::papokin::plugin::world::GenerationPhase::Features => {
                    generator.generate_features(&mut buffer);
                }
            }
        }
    }
}

/// 插件 API 全局使用的 `core::result::Result<T, String>` 的便捷别名。
pub type Result<T, E = String> = core::result::Result<T, E>;

/// 每个 Pumpkin 插件都必须实现的 trait。
///
/// 使用 [`register_plugin!`] 宏向运行时注册你的实现。
/// 生命周期与 IPC 回调使用共享引用，以便运行时可以安全地重入
/// 插件。会修改插件状态的实现必须使用线程安全的内部可变性。
/// 不要在持有不可重入锁的情况下调用宿主 API：该调用可能同步地把
/// 在原始调用返回之前，将事件或 IPC 回调重新送回同一个插件。
pub trait Plugin: Send + Sync {
    /// 创建插件的新实例。
    ///
    /// 由运行时在 [`on_load`](Plugin::on_load) 之前调用一次。
    fn new() -> Self
    where
        Self: Sized;

    /// 返回此插件的元数据。
    fn metadata(&self) -> PluginMetadata;

    /// 以 TOML 字符串形式返回插件的默认配置，供
    /// [`Context::load_config`](crate::Context::load_config) 来合并覆盖
    /// 已存储的配置文件。插件未附带配置时返回空字符串
    /// 默认值。
    fn config_defaults(&self) -> String {
        String::new()
    }

    /// 插件被服务器加载时调用。
    ///
    /// 用此方法注册事件处理器与命令，并执行任何设置工作。
    fn on_load(&self, _context: Context) -> Result<()> {
        Ok(())
    }

    /// 插件成功加载后调用，用于启用插件。
    ///
    /// 启用失败不会卸载插件：插件保持加载但
    /// 失活——其事件处理程序与命令会被注销。这对应
    /// 对应 Paper 的 `onEnable` 失败分级。
    fn on_enable(&self, _context: Context) -> Result<()> {
        Ok(())
    }

    /// 在 [`on_unload`](Plugin::on_unload) 之前调用，用于禁用活跃的插件。
    /// 在卸载与关停期间。
    ///
    /// 用此方法暂停工作并释放运行时资源，同时保留
    /// 保持插件已存储的数据完好无损。
    fn on_disable(&self, _context: Context) -> Result<()> {
        Ok(())
    }

    /// 插件被服务器卸载时调用。
    ///
    /// 用此方法清理在 [`on_load`](Plugin::on_load) 期间获取的任何资源。
    fn on_unload(&self, _context: Context) -> Result<()> {
        Ok(())
    }

    /// 插件收到来自其他插件的消息时调用。
    fn handle_ipc_message(
        &self,
        _sender: wit::PluginId,
        _message: wit::IpcMessage,
    ) -> Result<wit::IpcMessage, String> {
        Err("此插件无法接收消息。".to_string())
    }

    /// 当玩家在
    /// 通过消息接口注册的。
    ///
    /// 默认实现会丢弃该消息。
    fn on_plugin_message(&self, _player_uuid: &str, _channel: &str, _data: &[u8]) {}
}

#[doc(hidden)]
pub fn register_plugin(build_plugin: fn() -> Box<dyn Plugin>) {
    let _ = tracing::subscriber::set_global_default(WitSubscriber::new());
    assert!(
        PLUGIN.set(build_plugin()).is_ok(),
        "register_plugin 只能调用一次"
    );
}

///返回对当前已加载插件实例的引用。
///
/// # Panics
/// 若在 [`register_plugin`] 初始化 `PLUGIN` 之前调用。
fn plugin() -> &'static dyn Plugin {
    #[allow(clippy::expect_used)]
    PLUGIN
        .get()
        .map(Box::as_ref)
        .expect("必须先调用 register_plugin 初始化 PLUGIN 后才能使用")
}

/// 返回已注册插件的 [`Plugin::config_defaults`] 字符串。
pub(crate) fn config_defaults() -> String {
    plugin().config_defaults()
}

/// 由 [`register_plugin`] 初始化的插件单例实例。
static PLUGIN: OnceLock<Box<dyn Plugin>> = OnceLock::new();

/// 将所提供的类型注册为 Pumpkin 插件。
///
/// 此宏生成 WebAssembly 导出入口点，服务器用它来
/// 实例化插件。该类型必须实现 [`Plugin`] trait。
///
/// # Example
/// ```rust,ignore
/// register_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        #[unsafe(export_name = "init-plugin")]
        pub extern "C" fn __init_plugin() {
            $crate::register_plugin(|| Box::new(<$plugin_type as $crate::Plugin>::new()));
        }
    };
}
/// AI 与生物目标（goal）工具。
pub mod ai;
/// 持久化自定义数据容器（Bukkit 风格的 `PersistentDataHolder`）。
pub mod persistent_data;
pub use persistent_data::PersistentDataHolder;
/// 游戏规则定义与取值。
pub use wit::papokin::plugin::game_rules::{GameRule, GameRuleValue};
