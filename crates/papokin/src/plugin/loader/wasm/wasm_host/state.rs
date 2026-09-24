use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Weak},
};

use papokin_util::text::TextComponent;
use tokio::sync::Mutex;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::{
    RequestOptions, WasiBody, WasiHttpCtx, WasiHttpCtxView, WasiHttpHooks, WasiHttpView,
};

use crate::{
    command::CommandSender,
    entity::EntityBase,
    entity::player::Player,
    plugin::{
        Context,
        api::gui::PluginGui,
        loader::wasm::wasm_host::{WasmPlugin, args::OwnedArg},
    },
    server::{RecipeManager, Server},
    world::World,
};

pub struct WasmCommand {
    pub names: Vec<String>,
    pub builder: crate::command::argument_builder::CommandArgumentBuilder,
}

impl WasmCommand {
    #[must_use]
    pub fn new(names: Vec<String>, description: String) -> Self {
        let primary = names.first().cloned().unwrap_or_default();
        let builder = crate::command::argument_builder::command(primary, description);
        Self { names, builder }
    }

    #[must_use]
    pub fn then(mut self, child: WasmCommandNode) -> Self {
        use crate::command::argument_builder::ArgumentBuilder;
        self.builder = self.builder.then(child.into_detached_node());
        self
    }

    #[must_use]
    pub fn executes(
        mut self,
        executor: impl crate::command::node::CommandExecutor + 'static,
    ) -> Self {
        use crate::command::argument_builder::ArgumentBuilder;
        self.builder = self.builder.executes(executor);
        self
    }
}

pub enum WasmCommandNode {
    Literal(crate::command::argument_builder::LiteralArgumentBuilder),
    Argument(crate::command::argument_builder::RequiredArgumentBuilder),
}

impl WasmCommandNode {
    #[must_use]
    pub fn then(self, child: Self) -> Self {
        use crate::command::argument_builder::ArgumentBuilder;
        match self {
            Self::Literal(b) => Self::Literal(b.then(child.into_detached_node())),
            Self::Argument(b) => Self::Argument(b.then(child.into_detached_node())),
        }
    }

    #[must_use]
    pub fn executes(self, executor: impl crate::command::node::CommandExecutor + 'static) -> Self {
        use crate::command::argument_builder::ArgumentBuilder;
        match self {
            Self::Literal(b) => Self::Literal(b.executes(executor)),
            Self::Argument(b) => Self::Argument(b.executes(executor)),
        }
    }

    #[must_use]
    pub fn suggests(
        self,
        provider: impl crate::command::suggestion::provider::SuggestionProvider + 'static,
    ) -> Self {
        match self {
            Self::Literal(b) => Self::Literal(b),
            Self::Argument(b) => Self::Argument(b.suggests(provider)),
        }
    }

    #[must_use]
    pub fn into_detached_node(self) -> crate::command::node::detached::DetachedNode {
        use crate::command::argument_builder::ArgumentBuilder;
        match self {
            Self::Literal(b) => crate::command::node::detached::DetachedNode::Literal(b.build()),
            Self::Argument(b) => crate::command::node::detached::DetachedNode::Argument(b.build()),
        }
    }
}

pub struct WasmResource<T> {
    pub provider: T,
}

pub type ServerResource = WasmResource<Arc<Server>>;
pub type ContextResource = WasmResource<Arc<Context>>;
pub type PlayerResource = WasmResource<Arc<Player>>;
pub type JavaPlayerResource = WasmResource<Arc<Player>>;
pub type EntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type WorldResource = WasmResource<Arc<World>>;
pub type ChunkResource = WasmResource<(Arc<World>, Weak<papokin_world::chunk::ChunkData>)>;
pub type WorldBorderResource = WasmResource<Arc<World>>;

/// 支撑 WIT 的区块方块与生物群系数据宿主端副本
/// `chunk-snapshot` 资源（读取时复制、只读）。
pub struct ChunkSnapshot {
    pub x: i32,
    pub z: i32,
    pub min_y: i32,
    pub section_count: u32,
    /// 所有方块状态 ID，区块节优先，自底向上；在每个
    /// 区段按 Y 优先、再 Z、后 X 的顺序排列（每区段 4096 个条目）。
    pub blocks: Vec<u16>,
    /// 所有生物群系 ID，按 4x4x4 四分格分辨率，区块节优先，自底
    /// 之上；区块节内按 Y 优先、其次 Z、再次 X 排序（每个
    /// 区段）。
    pub biomes: Vec<u8>,
}

pub type ChunkSnapshotResource = WasmResource<ChunkSnapshot>;

#[derive(Clone)]
pub enum ScoreboardProvider {
    World(Arc<World>),
    Player(Arc<Player>),
}

pub type ScoreboardResource = WasmResource<ScoreboardProvider>;
pub type GuiResource = WasmResource<Arc<Mutex<PluginGui>>>;
pub type BossBarResource = WasmResource<
    Arc<Mutex<crate::plugin::loader::wasm::wasm_host::wit::v0_1::boss_bar::PluginBossBar>>,
>;
pub type TextComponentResource = WasmResource<TextComponent>;
pub type CommandResource = WasmResource<WasmCommand>;
pub type CommandSenderResource = WasmResource<CommandSender>;
pub type ConsumedArgsResource = WasmResource<HashMap<String, OwnedArg>>;
pub type CommandNodeResource = WasmResource<WasmCommandNode>;
pub type ItemStackResource = WasmResource<Arc<Mutex<papokin_data::item_stack::ItemStack>>>;
pub type RecipeManagerResource = WasmResource<Arc<RecipeManager>>;
pub type EnchantmentManagerResource =
    WasmResource<Arc<crate::server::enchantment::EnchantmentManager>>;
pub type OpManagerResource = WasmResource<Arc<Server>>;
pub type BanManagerResource = WasmResource<Arc<Server>>;
pub type WhitelistManagerResource = WasmResource<Arc<Server>>;
pub type DatapackManagerResource = WasmResource<Arc<Server>>;
pub type DamageTypeManagerResource =
    WasmResource<Arc<crate::server::damage_type::DamageTypeManager>>;
pub type TagManagerResource = WasmResource<Arc<crate::server::tag::TagManager>>;
pub type RegistryManagerResource = WasmResource<Arc<crate::server::registry::RegistryManager>>;
pub type BlockEntityResource = WasmResource<Arc<dyn crate::block::entities::BlockEntity>>;

#[derive(Clone)]
pub enum InventoryProvider {
    Generic(Arc<dyn papokin_inventory::Inventory>),
    PlayerMain(Arc<Player>),
    PlayerEnderChest(Arc<Player>),
}

pub type InventoryResource = WasmResource<InventoryProvider>;
pub type PlayerInventoryResource = WasmResource<Arc<Player>>;

pub type LivingEntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type MobResource = WasmResource<Arc<dyn EntityBase>>;

#[derive(Clone)]
pub struct ContainerBlockEntity {
    pub provider: Arc<dyn crate::block::entities::BlockEntity>,
    pub inventory: Arc<dyn papokin_inventory::Inventory>,
}

pub type ContainerBlockEntityResource = WasmResource<ContainerBlockEntity>;

pub type DisplayEntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type BlockDisplayEntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type ItemDisplayEntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type TextDisplayEntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type InteractionEntityResource = WasmResource<Arc<dyn EntityBase>>;
pub type MerchantResource = WasmResource<Arc<dyn EntityBase>>;
pub type MapViewResource =
    WasmResource<crate::plugin::loader::wasm::wasm_host::wit::v0_1::map::PluginMapView>;
pub type DragonFightResource =
    WasmResource<crate::plugin::loader::wasm::wasm_host::wit::v0_1::dragon::PluginDragonFight>;

#[derive(Clone)]
pub struct ChunkBuffer {
    pub x: i32,
    pub z: i32,
    pub min_y: i32,
    pub height: u32,
    pub proto_chunk: Arc<std::sync::Mutex<papokin_world::ProtoChunk>>,
}

pub type ChunkBufferResource = WasmResource<ChunkBuffer>;

pub type OwnedConsumedArgs = HashMap<String, OwnedArg>;

pub struct PluginHostState {
    pub wasi_ctx: WasiCtx,
    pub wasi_http_ctx: WasiHttpCtx,
    pub wasi_http_hooks: PluginHttpHooks,
    pub resource_table: ResourceTable,
    pub limits: wasmtime::StoreLimits,
    pub plugin: Option<Weak<WasmPlugin>>,
    pub server: Option<Arc<Server>>,
    pub permissions: Vec<String>,
    pub name: Option<String>,
    pub marketplace_metadata:
        Option<crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::context::MarketplaceMetadata>,
    /// 事件派发护栏：派发期间压入资源表条目的回收闭包。访客调用
    /// 失败（trap/panic）时参数所有权已降载给访客、无人释放，强引用
    /// 条目会无限累积（含 `Arc<Player>` 滞留已离线玩家）——失败时按
    /// 记录逐条回收兜底。成功路径由 `cleanup_event` 与访客 drop 按
    /// 正常生命周期释放，护栏只清空记录、绝不重放（访客可能持有
    /// 调用期间新建的长期句柄，重放会误删活句柄）。
    pub dispatch_guard: Vec<ResourceTableDeleter>,
    /// 护栏嵌套深度：大于零时 `add_*` 压入的条目被记录。
    dispatch_guard_depth: u32,
}

/// 按记录 rep 从资源表删除单条条目的回收闭包。
pub type ResourceTableDeleter = Box<dyn FnOnce(&mut ResourceTable) + Send>;

impl Default for PluginHostState {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHostState {
    /// 资源表硬上限：封顶恶意/劣质访客囤积句柄的内存面。到达后
    /// `push` 返回表满错误（事件路径经软限与 `catch_unwind` 双防线
    /// 优雅降级，其余路径按错误处理）。
    pub const RESOURCE_TABLE_MAX_CAPACITY: usize = 8192;
    /// 事件派发软限：到达后跳过后续派发并记日志，为单次派发预留
    /// 余量（大服 `PlayerChatEvent.recipients` 等大事件可达数百条）。
    pub const RESOURCE_TABLE_DISPATCH_SOFT_LIMIT: usize = 7168;

    #[must_use]
    pub fn new() -> Self {
        let mut resource_table = ResourceTable::new();
        resource_table.set_max_capacity(Self::RESOURCE_TABLE_MAX_CAPACITY);
        Self {
            wasi_ctx: WasiCtxBuilder::new()
                .inherit_stdout() // 允许打印消息与错误
                .inherit_stderr() // 在 `on_load` 之前，例如在元数据获取期间
                .build(),
            wasi_http_ctx: WasiHttpCtx::new(),
            wasi_http_hooks: PluginHttpHooks::new(),
            resource_table,
            limits: wasmtime::StoreLimitsBuilder::new().build(),
            plugin: None,
            server: None,
            permissions: Vec::new(),
            name: None,
            marketplace_metadata: None,
            dispatch_guard: Vec::new(),
            dispatch_guard_depth: 0,
        }
    }

    /// 事件派发前的软限检查：占用达到软限即应跳过派发（优雅降级
    /// 而非让降载中途 `push` 失败 panic）。空表走 `is_empty` 快速
    /// 路径，正常负载零遍历开销。
    #[must_use]
    pub fn resource_table_exceeds_dispatch_soft_limit(&mut self) -> bool {
        !self.resource_table.is_empty() && {
            let occupied = self.resource_table.iter_mut().count();
            occupied >= Self::RESOURCE_TABLE_DISPATCH_SOFT_LIMIT
        }
    }

    /// 进入事件派发：此后 `add_*` 压入的条目会被记录，供失败路径
    /// 回收。见 `dispatch_guard` 字段注释。
    pub const fn begin_dispatch_guard(&mut self) {
        self.dispatch_guard_depth = self.dispatch_guard_depth.saturating_add(1);
    }

    /// 派发成功：结束记录并丢弃记录（不重放，避免误删访客持有的
    /// 活句柄——资源已按正常生命周期释放）。
    pub fn end_dispatch_guard_success(&mut self) {
        self.dispatch_guard_depth = self.dispatch_guard_depth.saturating_sub(1);
        self.dispatch_guard.clear();
    }

    /// 派发失败（访客 trap/panic/宿主降载失败）：重放回收全部被
    /// 记录条目。已按正常路径释放过的条目删除为无害 no-op；访客
    /// trap 后残留的句柄即使被其后续使用也只得到干净的「无效句柄」
    /// 错误，而不是永久泄漏强引用。
    pub fn end_dispatch_guard_failure(&mut self) {
        self.dispatch_guard_depth = self.dispatch_guard_depth.saturating_sub(1);
        for deleter in std::mem::take(&mut self.dispatch_guard) {
            deleter(&mut self.resource_table);
        }
    }

    fn record_dispatch_guard<F>(&mut self, delete: F)
    where
        F: FnOnce(&mut ResourceTable) + Send + 'static,
    {
        if self.dispatch_guard_depth > 0 {
            self.dispatch_guard.push(Box::new(delete));
        }
    }

    pub fn add_server<T>(
        &mut self,
        provider: Arc<Server>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ServerResource { provider })?;
        let rep = resource.rep();
        self.record_dispatch_guard(move |table| {
            let _ = table.delete::<ServerResource>(wasmtime::component::Resource::new_own(rep));
        });
        Ok(wasmtime::component::Resource::new_own(rep))
    }

    pub fn add_context<T>(
        &mut self,
        provider: Arc<Context>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ContextResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_player<T>(
        &mut self,
        provider: Arc<Player>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(PlayerResource { provider })?;
        let rep = resource.rep();
        self.record_dispatch_guard(move |table| {
            let _ = table.delete::<PlayerResource>(wasmtime::component::Resource::new_own(rep));
        });
        Ok(wasmtime::component::Resource::new_own(rep))
    }

    pub fn add_java_player<T>(
        &mut self,
        provider: Arc<Player>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(JavaPlayerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(EntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_living_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(LivingEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_mob<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(MobResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_world<T>(
        &mut self,
        provider: Arc<World>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(WorldResource { provider })?;
        let rep = resource.rep();
        self.record_dispatch_guard(move |table| {
            let _ = table.delete::<WorldResource>(wasmtime::component::Resource::new_own(rep));
        });
        Ok(wasmtime::component::Resource::new_own(rep))
    }

    pub fn add_chunk<T>(
        &mut self,
        world: Arc<World>,
        chunk: Weak<papokin_world::chunk::ChunkData>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ChunkResource {
            provider: (world, chunk),
        })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_chunk_snapshot<T>(
        &mut self,
        provider: ChunkSnapshot,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(ChunkSnapshotResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_chunk_snapshot_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&ChunkSnapshotResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_world_border<T>(
        &mut self,
        provider: Arc<World>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(WorldBorderResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_scoreboard<T>(
        &mut self,
        provider: ScoreboardProvider,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ScoreboardResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_gui<T>(
        &mut self,
        provider: Arc<Mutex<PluginGui>>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(GuiResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_boss_bar<T>(
        &mut self,
        provider: Arc<
            Mutex<crate::plugin::loader::wasm::wasm_host::wit::v0_1::boss_bar::PluginBossBar>,
        >,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(BossBarResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_text_component<T>(
        &mut self,
        provider: TextComponent,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(TextComponentResource { provider })?;
        let rep = resource.rep();
        self.record_dispatch_guard(move |table| {
            let _ =
                table.delete::<TextComponentResource>(wasmtime::component::Resource::new_own(rep));
        });
        Ok(wasmtime::component::Resource::new_own(rep))
    }

    pub fn add_command<T>(
        &mut self,
        provider: WasmCommand,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(CommandResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_command_sender<T>(
        &mut self,
        command_sender: CommandSender,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(CommandSenderResource {
            provider: command_sender,
        })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_consumed_args<T>(
        &mut self,
        provider: HashMap<String, OwnedArg>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(ConsumedArgsResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_owned_consumed_args<T>(
        &mut self,
        provider: OwnedConsumedArgs,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(ConsumedArgsResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_command_node<T>(
        &mut self,
        provider: WasmCommandNode,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(CommandNodeResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_item_stack<T>(
        &mut self,
        provider: Arc<Mutex<papokin_data::item_stack::ItemStack>>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ItemStackResource { provider })?;
        let rep = resource.rep();
        self.record_dispatch_guard(move |table| {
            let _ = table.delete::<ItemStackResource>(wasmtime::component::Resource::new_own(rep));
        });
        Ok(wasmtime::component::Resource::new_own(rep))
    }

    pub fn add_recipe_manager<T>(
        &mut self,
        provider: Arc<RecipeManager>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(RecipeManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_enchantment_manager<T>(
        &mut self,
        provider: Arc<crate::server::enchantment::EnchantmentManager>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(EnchantmentManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_op_manager<T>(
        &mut self,
        provider: Arc<Server>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(OpManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_ban_manager<T>(
        &mut self,
        provider: Arc<Server>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(BanManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_whitelist_manager<T>(
        &mut self,
        provider: Arc<Server>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(WhitelistManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_datapack_manager<T>(
        &mut self,
        provider: Arc<Server>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(DatapackManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_damage_type_manager<T>(
        &mut self,
        provider: Arc<crate::server::damage_type::DamageTypeManager>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(DamageTypeManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_tag_manager<T>(
        &mut self,
        provider: Arc<crate::server::tag::TagManager>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(TagManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_registry_manager<T>(
        &mut self,
        provider: Arc<crate::server::registry::RegistryManager>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(RegistryManagerResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_inventory<T>(
        &mut self,
        provider: InventoryProvider,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(InventoryResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_player_inventory<T>(
        &mut self,
        provider: Arc<Player>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(PlayerInventoryResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_block_entity<T>(
        &mut self,
        provider: Arc<dyn crate::block::entities::BlockEntity>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(BlockEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_container_block_entity<T>(
        &mut self,
        provider: Arc<dyn crate::block::entities::BlockEntity>,
        inventory: Arc<dyn papokin_inventory::Inventory>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ContainerBlockEntityResource {
            provider: ContainerBlockEntity {
                provider,
                inventory,
            },
        })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn add_display_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(DisplayEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_display_entity_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&DisplayEntityResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_block_display_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(BlockDisplayEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_block_display_entity_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&BlockDisplayEntityResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_item_display_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(ItemDisplayEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_item_display_entity_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&ItemDisplayEntityResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_text_display_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(TextDisplayEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_text_display_entity_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&TextDisplayEntityResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_interaction_entity<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self
            .resource_table
            .push(InteractionEntityResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_interaction_entity_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&InteractionEntityResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_merchant<T>(
        &mut self,
        provider: Arc<dyn EntityBase>,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(MerchantResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_merchant_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&MerchantResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_map_view<T>(
        &mut self,
        provider: crate::plugin::loader::wasm::wasm_host::wit::v0_1::map::PluginMapView,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(MapViewResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_map_view_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&MapViewResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_dragon_fight<T>(
        &mut self,
        provider: crate::plugin::loader::wasm::wasm_host::wit::v0_1::dragon::PluginDragonFight,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(DragonFightResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_dragon_fight_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&DragonFightResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }

    pub fn add_chunk_buffer<T>(
        &mut self,
        provider: ChunkBuffer,
    ) -> wasmtime::Result<wasmtime::component::Resource<T>> {
        let resource = self.resource_table.push(ChunkBufferResource { provider })?;
        Ok(wasmtime::component::Resource::new_own(resource.rep()))
    }

    pub fn get_chunk_buffer_res<T>(
        &self,
        resource: &wasmtime::component::Resource<T>,
    ) -> wasmtime::Result<&ChunkBufferResource> {
        Ok(self
            .resource_table
            .get(&wasmtime::component::Resource::new_borrow(resource.rep()))?)
    }
}

pub struct PluginHttpHooks {
    pub allow_outbound: bool,
}

impl PluginHttpHooks {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allow_outbound: false,
        }
    }
}

impl Default for PluginHttpHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiHttpHooks for PluginHttpHooks {
    fn send_request(
        &mut self,
        request: hyper::Request<WasiBody>,
        options: Option<RequestOptions>,
        fut: Box<dyn Future<Output = wasmtime_wasi_http::Result<()>> + Send>,
    ) -> Box<
        dyn Future<
                Output = wasmtime_wasi_http::Result<(
                    hyper::Response<WasiBody>,
                    Box<dyn Future<Output = wasmtime_wasi_http::Result<()>> + Send>,
                )>,
            > + Send,
    > {
        if !self.allow_outbound {
            return Box::new(async { Err(wasmtime_wasi_http::Error::HttpRequestDenied) });
        }

        wasmtime_wasi_http::default_hooks().send_request(request, options, fut)
    }
}

impl WasiView for PluginHostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

impl WasiHttpView for PluginHostState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.wasi_http_ctx,
            table: &mut self.resource_table,
            hooks: &mut self.wasi_http_hooks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 派发护栏语义：成功路径只清记录不删条目（访客可能合法持有
    /// 调用期间获得的句柄）；失败路径重放回收全部被记录条目，且
    /// 对已删除条目无害。防止访客 trap 时事件参数资源（含强
    /// `Arc<Player>`）无限累积刷满资源表。
    #[test]
    fn dispatch_guard_success_keeps_failure_replays() {
        let mut state = PluginHostState::new();
        // 护栏之外压入的条目不被记录（长期句柄）
        let long_lived = state
            .add_text_component::<TextComponentResource>(TextComponent::text("常驻"))
            .unwrap();

        state.begin_dispatch_guard();
        let dispatched = state
            .add_text_component::<TextComponentResource>(TextComponent::text("事件"))
            .unwrap();
        state.end_dispatch_guard_success();
        // 成功路径：条目仍在（由 cleanup_event/访客 drop 释放）
        assert!(
            state
                .resource_table
                .get::<TextComponentResource>(&dispatched)
                .is_ok()
        );

        state.begin_dispatch_guard();
        let leaked = state
            .add_text_component::<TextComponentResource>(TextComponent::text("失败派发"))
            .unwrap();
        state.end_dispatch_guard_failure();
        // 失败路径：被记录条目被重放回收
        assert!(
            state
                .resource_table
                .get::<TextComponentResource>(&leaked)
                .is_err()
        );
        // 未记录条目不受失败重放影响
        assert!(
            state
                .resource_table
                .get::<TextComponentResource>(&long_lived)
                .is_ok()
        );
        // 护栏清空后可重复使用
        assert!(state.dispatch_guard.is_empty());
        assert_eq!(state.dispatch_guard_depth, 0);
    }

    /// 硬上限封顶：达到 `RESOURCE_TABLE_MAX_CAPACITY` 后 push 失败，
    /// 恶意访客无法无限囤积句柄（内存封顶）。
    #[test]
    fn resource_table_hard_capacity_caps_hoarding() {
        let mut state = PluginHostState::new();
        for _ in 0..PluginHostState::RESOURCE_TABLE_MAX_CAPACITY {
            let _resource = state
                .resource_table
                .push(TextComponentResource {
                    provider: TextComponent::text("囤积"),
                })
                .expect("上限内 push 必须成功");
        }
        assert!(
            state
                .resource_table
                .push(TextComponentResource {
                    provider: TextComponent::text("溢出"),
                })
                .is_err(),
            "达到硬上限后 push 必须失败"
        );
    }

    /// 软限检查：占用低于软限时放行（含空表快速路径），压满软限
    /// 后应触发跳过派发。
    #[test]
    fn dispatch_soft_limit_skips_when_table_is_full() {
        let mut state = PluginHostState::new();
        assert!(!state.resource_table_exceeds_dispatch_soft_limit());
        for _ in 0..PluginHostState::RESOURCE_TABLE_DISPATCH_SOFT_LIMIT {
            let _resource = state
                .resource_table
                .push(TextComponentResource {
                    provider: TextComponent::text("占位"),
                })
                .expect("软限低于硬上限，push 必须成功");
        }
        assert!(state.resource_table_exceeds_dispatch_soft_limit());
    }
}
