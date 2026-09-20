# Papokin 插件 API 文档

> 版本：`pumpkin:plugin@0.1.0`（WIT 契约）· `PLUGIN_API_VERSION = 3` · 适用 MC Java **1.21.11**
> SDK：`crates/pumpkin-plugin-api` · 契约：`crates/pumpkin-plugin-wit/v0.1`（51 个 WIT 文件）
> 实施背景见 [11-插件API强化实现记录](11-插件API强化实现记录-Papo机制级覆盖.md)；本文是插件开发者的**参考文档**。
> 文中示例均取自/对齐已实跑验证的 `examples/e2e-plugin`（7 个日志标记基线）。

---

## 目录

1. [架构总览](#一架构总览)
2. [快速上手](#二快速上手)
3. [插件生命周期与元数据](#三插件生命周期与元数据)
4. [事件系统](#四事件系统)
5. [任务调度器](#五任务调度器)
6. [命令](#六命令)
7. [权限（双层模型）](#七权限双层模型)
8. [配置文件](#八配置文件)
9. [插件间通信（服务 / IPC / 插件消息）](#九插件间通信)
10. [数据存储](#十数据存储)
11. [世界 / 实体 / 玩家操作](#十一世界--实体--玩家操作)
12. [自定义 AI 目标](#十二自定义-ai-目标)
13. [自定义世界生成](#十三自定义世界生成)
14. [其他子系统速览](#十四其他子系统速览)
15. [沙箱与日志](#十五沙箱与日志)
16. [附录：API 面统计与版本策略](#十六附录)

---

## 一、架构总览

Papokin 插件是 **WASM 组件（Component Model）**，不是 JVM 字节码。三层结构：

```
┌────────────────────────────────────────────────────┐
│ 你的插件（Rust → wasm32-wasip2 组件，0x1000d）        │
│  依赖 pumpkin-plugin-api（SDK，对 WIT 的安全封装）     │
├────────────────────────────────────────────────────┤
│ WIT 契约 pumpkin:plugin@0.1.0                       │
│  宿主→插件：imports（scheduler/services/world…51 接口）│
│  插件→宿主：exports（on-load / handle-event …）       │
├────────────────────────────────────────────────────┤
│ 服务器宿主（wasmtime），事件分发 / 调度 / 权限 / 沙箱    │
└────────────────────────────────────────────────────┘
```

- 插件运行在 **WASI 沙箱**内：默认无文件系统、无网络、无环境变量；一切能力通过声明式权限申请（见 §七、§十五）。
- SDK 只是 WIT 绑定 + 便利封装：你在 SDK 里看到的每个类型，背后都是一次宿主调用。
- WIT 变更（加接口/函数）会自动传导：SDK 用 `wit_bindgen::generate!`、宿主用 `wasmtime::component::bindgen!`，都从 `crates/pumpkin-plugin-wit/v0.1` 即时生成。

---

## 二、快速上手

### 2.1 最小插件

```rust
use pumpkin_plugin_api::{Context, LoadOrder, Plugin, PluginMetadata, Result, register_plugin};

struct MyPlugin;

impl Plugin for MyPlugin {
    fn new() -> Self { Self }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "my-plugin".into(),
            version: "0.1.0".into(),
            authors: vec!["you".into()],
            description: "An example plugin.".into(),
            dependencies: vec![],        // 硬依赖：缺失则拒载
            permissions: vec![],         // 沙箱能力申请（§七）
            load_after: vec![],          // 软排序边
            load_before: vec![],
            provides: vec![],            // 能力别名
            load_order: LoadOrder::PostWorld,
        }
    }

    fn on_load(&self, _context: Context) -> Result<()> {
        tracing::info!("hello from my-plugin");
        Ok(())
    }
}

register_plugin!(MyPlugin);
```

### 2.2 Cargo 配置（非 workspace 成员）

```toml
# Cargo.toml
[workspace]

[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
pumpkin-plugin-api = { path = "<papokin>/crates/pumpkin-plugin-api" }
tracing = "0.1"
```

> **必须**把插件 crate 置于独立 workspace（空 `[workspace]` 表），否则会被主 workspace 的依赖图吞掉。

### 2.3 构建与部署

```bash
rustup target add wasm32-wasip2          # 首次
cargo build --target wasm32-wasip2       # 产出 target/wasm32-wasip2/debug/<name>.wasm
cp target/wasm32-wasip2/debug/my_plugin.wasm <服务器>/plugins/
```

启动服务器即加载；日志里插件的 `tracing` 输出会自动附带 `plugin.target` / `plugin.module` 字段。

---

## 三、插件生命周期与元数据

### 3.1 生命周期回调

| 回调 | 时机 | 失败语义 |
|---|---|---|
| `fn on_load(&self, Context) -> Result<()>` | 插件被加载 | **load 失败 → 整体卸载**，不进入后续阶段 |
| `fn on_enable(&self, Context) -> Result<()>` | load 成功后（含重启用） | enable 失败 → **停用不卸载**：注销其事件 handler / 命令 / 服务，状态置 Disabled，可再次 enable |
| `fn on_disable(&self, Context) -> Result<()>` | 卸载前 / 关服时 / 被停用 | — |
| `fn on_unload(&self, Context) -> Result<()>` | 插件被移除 | — |

约定：**on_load 做静态注册准备，on_enable 里注册事件/命令/服务并开始工作，on_disable 暂停工作释放运行时资源，on_unload 保存数据。**

### 3.2 加载相位（LoadOrder）

```rust
pub enum LoadOrder { Startup, PostWorld }
```

- `PostWorld`（默认）：世界就绪后加载，绝大多数插件用这个。
- `Startup`：**在任何世界创建之前**（`Server::new` 引导阶段）加载。此阶段 `on_load`/`on_enable` 里不得触碰世界与玩家。
- 加载顺序由依赖图拓扑排序决定；跨相位依赖边会被正确裁剪；已加载插件在二次扫描时跳过。

### 3.3 依赖声明

| 字段 | 语义 |
|---|---|
| `dependencies: Vec<String>` | **硬依赖**：任一缺失 → 本插件拒绝加载 |
| `load_after: Vec<String>` | 软边：存在则排其后，缺失忽略 |
| `load_before: Vec<String>` | 软边：存在则排其前，缺失忽略 |
| `provides: Vec<String>` | 能力别名：别的插件可依赖这个别名而不是你的具体名字 |

### 3.4 签名与市场元数据（可选）

插件二进制可带签名。宿主提供：

```rust
context.get_marketplace_metadata() -> Option<MarketplaceMetadata>
// 含 marketplace_url / plugin_id / version / dev_id / is_paid / license_key / issued_at
```

未签名插件返回 `None`。

---

## 四、事件系统

### 4.1 注册 handler

```rust
use pumpkin_plugin_api::events::{EventHandler, EventData, EventPriority, PlayerJoinEvent};

type JoinData = EventData<PlayerJoinEvent>;   // EventData<E> = E 的数据 record

struct JoinAnnouncer;
impl EventHandler<PlayerJoinEvent> for JoinAnnouncer {
    fn handle(&self, server: pumpkin_plugin_api::Server, event: JoinData) -> JoinData {
        // event.player / event.join_message / event.cancelled …
        event                                  // 原样返回 = 不修改事件
    }
}

// 在 on_enable 中：
context.register_event_handler(
    JoinAnnouncer,
    EventPriority::Normal,  // highest | high | normal | low | lowest
    true,                   // blocking：同步等待本 handler 完成
    false,                  // ignore_cancelled：false = 已取消事件也照常收到
)?;
```

### 4.2 分发语义（Bukkit 兼容）

1. **优先级排序**：`Lowest` 先执行 → `Highest` 最后执行（同优先级按注册序，稳定排序）。
2. **两阶段**：所有 blocking handler 先全部执行，然后才是 non-blocking handler。
3. **ignoreCancelled**：`ignore_cancelled = true` 的 handler 在事件已取消时被跳过。
4. **可取消事件**：数据 record 带 `cancelled: bool` 字段（由 `#[cancellable]` 宏注入）。blocking handler 把 `cancelled` 置 `true` 返回即可取消事件的后续效果（如爆炸不破坏方块、TNT 不引爆）。

### 4.3 事件目录

WIT `event.wit` 的 `event` variant 定义了 **273 种事件类型**，覆盖：

| 域 | 示例 |
|---|---|
| 玩家 | PlayerJoin/Leave/Login/PreLogin、AsyncPlayerChat（可改 format）、PlayerCommandPreprocess、PlayerMove/Teleport、PlayerPortal、PlayerExpCooldownChange、PlayerShow/HideEntity、PlayerTakeLecternBook |
| 实体 | CreatureSpawn（含 BREEDING/NATURAL/CHUNK_GENERATION 原因）、EntityDamage/ByEntity/ByBlock、EntityDeath、EntityCombust*、EntityKnockback*、EntityPortalEnter/Exit、EntitySpellCast、SlimeSplit、SheepDyeWool |
| 方块/世界 | BlockBreak/Place、BlockFade、BlockMultiPlace（多格）、BlockDispense*、BlockReceiveGameEvent、FluidLevelChange、CauldronLevelChange、ChunkLoad/Send、TNTPrime（5 种点火原因）、BellRing、MapInitialize |
| 交互 | HangingBreak/ByEntity、ItemMerge、PlayerInteract 系、Inventory* 系 |
| 服务端 | ServerTickStart、GameEvent、Raid*、Vehicle* |

已知**不可接线**的 9 个事件（底层 vanilla 机制缺失，见 note/11 §三）：SculkBloom、BellResonate、VaultDisplayItem、EntityBlockForm、EntityTargetBlock、ExpBottle、HorseJump、ArrowBodyCountChange、PlayerArmorStandManipulate。

---

## 五、任务调度器

WIT `scheduler` 接口（7 个函数）。SDK 两条入口：

- `SchedulerExt`（实现在 `Context` 和 `Server` 上）：通用任务
- `EntitySchedulerExt`（实现在 `Entity` 上）：实体生命周期绑定任务

| 方法 | 时基 | 行为 |
|---|---|---|
| `schedule_delayed_task(ticks, handler)` | 游戏刻 | 延迟一次性 |
| `schedule_repeating_task(ticks, period, handler)` | 游戏刻 | 延迟后周期执行 |
| `schedule_async_delayed_task(ms, handler)` | 墙钟毫秒 | **异步执行器**上一次性（不占 tick） |
| `schedule_async_repeating_task(ms, period, handler)` | 墙钟毫秒 | 异步执行器上周期执行 |
| `Entity::schedule_entity_delayed_task(ticks, handler)` | 游戏刻 | **绑定实体**：实体消失则静默跳过 |
| `Entity::schedule_entity_repeating_task(ticks, period, handler)` | 游戏刻 | 同上；实体消失时永久停止 |
| `cancel_task(task_id)` | — | 取消任意以上任务 |

```rust
use pumpkin_plugin_api::scheduler::{SchedulerExt, cancel_task};

// 1 秒后在主 tick 循环执行
let id = context.schedule_delayed_task(20, |server| {
    server.log("one second passed");
});
cancel_task(id); // 也可以取消

// 墙钟异步任务（Bukkit runTaskLaterAsynchronously）
context.schedule_async_delayed_task(300, |_server| {
    tracing::info!("300ms later on the async executor");
});
```

实体绑定任务（Bukkit EntityScheduler 语义）：

```rust
use pumpkin_plugin_api::scheduler::EntitySchedulerExt;

// event.entity 是事件里拿到的 Entity 资源
entity.schedule_entity_repeating_task(10, 10, |_server| {
    // 每 10 tick 执行；实体退场后宿主自动停发，插件无需手动清理
});
```

> 注意：handler 可能被**重入**，捕获的可变状态需用线程安全内部可变性（`Mutex`/`Atomic*`）。同步任务回调发生在 tick 循环，不得阻塞；耗时工作请用 async 族。

---

## 六、命令

### 6.1 定义与注册

命令 = 名字（首名为主命令，其余为别名）+ 参数树（Brigadier 风格 `CommandNode`）+ 执行/建议 handler：

```rust
use pumpkin_plugin_api::command::{Command, CommandNode};
use pumpkin_plugin_api::commands::{CommandHandler, CommandSuggestionHandler};

struct GreetHandler;
impl CommandHandler for GreetHandler {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs)
        -> Result<i32, CommandError> {
        // args.get_value("player") -> Arg（variant，涵盖 bool/num/pos/players/… 全类型）
        sender.send_message(/* text-component */);
        Ok(0)                       // 退出码；Err 则把失败信息发给发送者
    }
}

let cmd = Command::new(vec!["greet".into(), "hi".into()], "向玩家问好") // greet + 别名 hi
    .then(CommandNode::argument("player", ArgumentType::Players))      // 参数树
    .then(CommandNode::literal("now"))
    .execute(GreetHandler);                                            // 挂执行 handler

context.register_command(cmd, "myplugin.greet")?;   // 第二参 = 所需权限节点
```

`CommandNode::literal(name)` / `CommandNode::argument(name, ArgumentType)`，用 `.then(node)` 组树；`ArgumentType` 覆盖 40+ vanilla 参数类型（bool/数值区间/字符串、entity/players/game-profile、block-pos/position3d、block-state/predicate、item/predicate、nbt、particle、scoreboard、item-slot、资源/标签/枚举补全…）。

### 6.2 补全建议

```rust
struct TabCompleter;
impl CommandSuggestionHandler for TabCompleter {
    fn suggest(&self, sender: CommandSender, server: Server, request: SuggestionRequest)
        -> CommandSuggestions {
        // request.input / cursor / start / remaining
        CommandSuggestions {
            start: request.start,
            length: 0,
            values: vec![],   // CommandSuggestion { value, tooltip }
        }
    }
}
// cmd.suggest(TabCompleter);   // builder 风格，可与 .execute 链式
```

### 6.3 命名空间 fallback

注册同名命令发生冲突时，宿主自动改名为 `<plugin>:<label>`（如 `my-plugin:greet`），玩家可通过带前缀形式调用——与 Bukkit 的 namespaced registration 一致。`CommandSender` 资源提供 `is_player/is_console/as_player/permission_level/has_permission/position/world/locale` 等发送者查询。

---

## 七、权限（双层模型）

### 7.1 沙箱能力权限（插件 → 宿主）

声明在 `PluginMetadata.permissions`，控制**插件自身**能做什么（WASI 能力模型）：

```rust
use pumpkin_plugin_api::permissions;

permissions: vec![
    permissions::FS_READ_DATA.into(),   // 读私有数据文件夹
    permissions::FS_WRITE_DATA.into(),  // 写私有数据文件夹
    // 网络：NETWORK_DNS / NETWORK_TCP / NETWORK_UDP / HTTP_OUTBOUND …
    // 系统：SYS_ENV / SYS_INFO …（常量清单见 SDK permissions.rs）
],
```

未申请的能力在沙箱层直接拒绝。

### 7.2 游戏内权限节点（玩家 → 命令/功能）

面向**玩家/命令发送者**：

```rust
context.register_permission(Permission {
    node: "myplugin.greet".into(),
    description: "允许使用 /greet".into(),
    default: PermissionDefault::Op(PermissionLevel::Two), // deny | allow | op(0-4)
    children: vec![],
})?;
```

- 权限级别 0-4：`zero(普通) / one(moderator) / two(gamemaster) / three(admin) / four(owner)`。
- 服务器管理员可用 **`permissions.toml`**（服务端根目录）覆盖声明：default 支持 `true/false/deny/allow/op/op:<0-4>`，并可按玩家 UUID 授予/拒绝。启动时加载。

---

## 八、配置文件

Bukkit `getConfig/saveConfig` 的等价物，宿主代管文件（插件不直接碰 FS）：

```rust
impl Plugin for MyPlugin {
    // 默认值：TOML 文档字符串
    fn config_defaults(&self) -> String {
        "message = \"hello\"\n\n[bonus]\nenabled = true\n".into()
    }
}

// 运行时：加载 = 默认值 ⊔ 磁盘文件（深合并，用户值胜出），合并结果回写磁盘并返回
let config: String = context.load_config()?;
assert!(config.contains("hello") && config.contains("[bonus]"));
```

- 配置落盘位置：`plugins/data/<插件名>/config.toml`（由宿主构造路径，绕过沙箱 FS 权限）。
- 升级插件新增默认键 → 自动出现在合并结果；用户改过的值 → 永不丢失。
- `save_config(content)` 直接覆写（需合法 TOML）。

---

## 九、插件间通信

三个互补机制：

| 机制 | 方向 | 典型场景 |
|---|---|---|
| **Services**（ServicesManager） | 发现：谁提供 `my:economy`？ | 能力注册表 + 优先级 |
| **IPC** | 插件 ↔ 插件，同步请求/响应 | 调用另一插件的 API |
| **Plugin Messages** | 服务器 ↔ **客户端玩家** | 客户端 mod 联动（`minecraft:*` 保留） |

### 9.1 服务注册与发现

```rust
use pumpkin_plugin_api::services::ServiceProvider;

context.register_service("my-plugin:economy", 5)?;   // service 名 + 优先级
if let Some(ServiceProvider { plugin, priority, .. }) = context.get_service_provider("my-plugin:economy") {
    tracing::info!("economy provided by {plugin} (priority {priority})");
}
let all: Vec<ServiceProvider> = context.get_service_providers("my-plugin:economy"); // 按优先级降序
context.unregister_service("my-plugin:economy");
```

- 重复注册同一服务 = 更新优先级；插件 disable/unload 时注册自动移除。
- **发现窗口**：处于 Loading 相位的插件即可被 `get_service_provider` 发现——依赖方在 `on_enable` 里就能发现尚未 enable 完成的被依赖方。

### 9.2 IPC（同步调用另一插件）

```rust
use pumpkin_plugin_api::ipc;

// payload 自定义编码（如 JSON/bincode）；调用同步返回
let reply: Vec<u8> = ipc::send_ipc_message("other-plugin", payload)??;
// 外层 Result = 传输层（对端不存在/失败）；内层 Result = 对端业务结果
```

对端实现 `Plugin::handle_ipc_message`（默认返回错误）：

```rust
fn handle_ipc_message(&self, sender: PluginId, message: IpcMessage)
    -> Result<IpcMessage, String> {
    Ok(decode_and_answer(message)?)   // 返回 (可能为空的) 响应
}
```

### 9.3 插件消息通道（客户端联动）

```rust
context.register_incoming_channel("pumpkin:e2e")?;   // minecraft:* 为保留前缀，注册会报错
context.send_plugin_message(player_uuid, "pumpkin:e2e", b"payload".to_vec())?; // 仅 Java 版玩家
```

玩家在注册过的频道上发来消息时回调：

```rust
fn on_plugin_message(&self, player_uuid: &str, channel: &str, data: &[u8]) { /* … */ }
```

---

## 十、数据存储

### 10.1 私有数据文件夹（重启存续）

每个插件有私有数据文件夹；**声明 FS 权限后用普通文件 API 读写**（WASI 沙箱视角的路径）：

```rust
// metadata.permissions 需含 FS_READ_DATA / FS_WRITE_DATA
fn on_load(&self, context: Context) -> Result<()> {
    let path = format!("{}/state.json", context.get_data_folder());
    let saved = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(())
}
```

### 10.2 持久化数据容器（Bukkit PersistentDataHolder）

对实体 / 玩家 / 方块实体 / 区块 / 世界 / 物品栈挂**自定义 NBT 数据**，随存档自动持久化。实现 `PersistentDataHolder` trait 的类型（`ItemStack / Entity / BlockEntity / Chunk / World / Player`）：

```rust
use pumpkin_plugin_api::persistent_data::PersistentDataHolder;

entity.set_string("myplugin", "title", "屠龙者");     // 类型化便捷方法
entity.set_int("myplugin", "killcount", 42);
let kills = entity.get_int("myplugin", "killcount");  // Option<i32>
entity.remove_custom_data("myplugin", "killcount");
entity.has_custom_data("myplugin", "title");          // bool
```

- 底层是 `set_custom_data / get_custom_data`（裸 `NbtTree`）；类型化方法有 `set/get_string、int、long、short、byte、bool、float、double、byte_array、int_array、long_array`。
- 构造辅助：`persistent_data::{string_tree, int_tree, long_tree, …}`（返回 `NbtTree`）。
- namespace 用插件名，避免与其他插件冲突。

---

## 十一、世界 / 实体 / 玩家操作

WIT 侧最大的一块 API 面（函数数：`world` 252 · `block-entity` 172 · `player` 127 · `item-stack` 28 · `inventory` 28 · `server` 64）。

### 11.1 Server（64 个方法）

- **玩家**：`get_all_players / get_player_by_name / get_player_by_uuid`、`get_player_count`、`get_players_in_world`
- **世界**：`get_all_worlds / get_world_by_name / has_world / create_world / unload_world / save_all`
- **管理器**：`get_op_manager / get_ban_manager / get_whitelist_manager`（封禁/白名单/OP 全套 CRUD）、`get_recipe_manager / get_enchantment_manager / get_advancement / get_datapack_manager`
- **运行状态**：`get_mspt / get_tps / get_difficulty / get_max_players / get_motd / get_view_distance / get_simulation_distance / get_default_gamemode`
- **操作**：`execute_command(cmd, sender)`、`broadcast(message)`、`set_server_links`、`delete_message_*`（聊天签名删除）

### 11.2 World

- 方块：`get_block_state / set_block_state / set_block_by_name / get_block / set_block`（`BlockFlags` 控制更新）
- 区块：`get_chunk(x, z)`；`get_top_block_y / get_motion_blocking_height` 高度查询
- 环境：`get/set_time_of_day`、`is_raining / set_raining / is_thundering`、`get_spawn_location`、`get_border`（世界边界）
- 表现：`broadcast_system_message`、`play_sound`、`get_scoreboard`
- 尺寸：`get_dimension`

> **实体查询边界**：World 级**没有**按 ID/UUID 枚举实体的接口——实体资源只能从事件参数、`Entity::get_nearby_entities`（邻域查询）、车辆乘客链、`Player` 事件等获得。这是 EntityScheduler 一节所述无头验证限制的根因。

### 11.3 Entity / LivingEntity / Mob

- 通用（entity 资源）：位置/朝向/速度、`teleport`、fire ticks、fall distance、custom name、invulnerable、pose、vehicle/passengers、`as_living` 向下转型、`get_nearby_entities(x,y,z)`、`get_target_entity(max_distance)` 射线选实体
- LivingEntity：血量、装备、药水效果、伤害/击杀路径
- Mob：AI 目标（§十二）、`set_target` 目标选择
- 类型特化数据（`mobs.rs`）：`CreeperData / SlimeData / VillagerData / WolfData / SheepData …`，配套 `MobCast` 下转型 + `get_data/set_data`

### 11.4 Player

127 个方法：物品栏/末影箱（`PlayerEnderChestExt`）、冷却（`PlayerCooldownExt`）、计分板队伍（`PlayerTeamExt`）、属性（attributes）、药水/状态效果、表单/对话框（forms / java-dialogs）、声音/粒子、传送、权限检查等。

### 11.5 方块与物品

- `block.rs`：`BlockType / BlockStateTypeExt / IntoBlockKey`
- `item.rs`：`Item / IntoItemKey / ItemStackExt`（含 `ItemAttributeModifier`）
- `block-entity`（172 函数）：箱子/熔炉/讲台等容器方块实体的读写
- `data-components`：1.21 数据组件面
- `enchantments / potions / status-effect / damage-types / entity-statuses / statistics / recipe / advancement`：对应 vanilla 系统的查询与操作

---

## 十二、AI 目标

Mob 资源提供内建 AI 目标操作（`world.wit` 的 `builtin-ai-goal` variant：攻击/游荡/恐慌等 vanilla 目标集）：

```rust
mob.add_ai_goal(priority_u8, builtin_goal);   // 添加内建目标
mob.clear_ai_goals();                          // 清空
```

**自定义目标**：SDK 提供 `AiGoal` trait（`ai.rs`），回调均以共享引用调用（可重入，可变状态用内部可变性）：

```rust
use pumpkin_plugin_api::ai::AiGoal;

struct MyGoal;
impl AiGoal for MyGoal {
    fn can_start(&self, server: Server, entity: Entity) -> bool { false }
    fn should_continue(&self, server: Server, entity: Entity) -> bool { false }
    fn start(&self, server: Server, entity: Entity) {}
    fn tick(&self, server: Server, entity: Entity) {}   // 活跃期间每 tick
    fn stop(&self, server: Server, entity: Entity) {}
}
```

> **已知 SDK 缺口（诚实声明）**：`Mob` 资源有 `add_custom_ai_goal(priority, goal_id)` 挂载点，但 SDK 目前**没有公开的 `AiGoal` 注册函数**来获得 `goal_id`（注册表 `AI_GOAL_HANDLERS` 为 crate 内部）。宿主回调链路（`handle-ai-goal-*` 导出）已就绪；补一个公开注册函数即可启用，见 note/11 遗留项。

---

## 十三、自定义世界生成

```rust
use pumpkin_plugin_api::worldgen::{ChunkGenerator, ChunkBuffer, GeneratorManager};

struct FlatWorld;
impl ChunkGenerator for FlatWorld {
    fn generate_biomes(&self, chunk: &mut ChunkBuffer) { /* 步骤 1：生物群系 */ }
    fn generate_noise(&self, chunk: &mut ChunkBuffer) { /* 步骤 2：地形噪声 */ }
    fn generate_surface(&self, chunk: &mut ChunkBuffer) { /* 步骤 3：表面规则 */ }
    fn generate_features(&self, chunk: &mut ChunkBuffer) { /* 步骤 4：地物/矿藏 */ }
}

let id = GeneratorManager::register(FlatWorld);
world.set_chunk_generator(id);   // 挂到目标世界
```

`ChunkBuffer`（按区块本地坐标操作）：

- 坐标/范围：`x() / z() / min_y() / height()`
- 方块：`get_block(x,y,z) / set_block(x,y,z,state_id) / fill_layer(y,state_id) / fill_range(x,min_y,max_y,z,state_id) / fill_cuboid(...)`
- 生物群系：`set_biome(x,y,z,biome) / fill_biome(biome)`

宿主通过 `handle-generate-phase` 导出按相位回调插件。配合 `datapack` 接口可查询/启用数据包函数（`DatapackManager`，含计划函数队列）。

---

## 十四、其他子系统速览

| SDK 模块 | 内容 |
|---|---|
| `team.rs` | 计分板队伍：`Team / TeamSettingsBuilder / PlayerTeamExt / ScoreboardTeamExt` |
| `scoreboard`（WIT，28 函数） | 目标/分数/显示槽 |
| `boss-bar`（14 函数） | Boss 血条创建与更新 |
| `forms` / `java-dialogs` | 表单 UI（含 Bedrock 表单）与 1.21.6+ Java 对话框 |
| `display.rs` | 展示实体（text display 等）builder |
| `gui`（11 函数） | 界面相关 |
| `inventory.rs` | `Inventory / PlayerInventory` 抽象 |
| `recipe` | 配方查询/注册 |
| `advancement` | 进度（`AdvancementDisplay / AdvancementProgress / FrameType`） |
| `datapack.rs` | `DatapackInfo / DatapackManager / EnablePosition` |
| `i18n` | 服务端翻译键（**按客户端版本翻译**） |
| `text.rs` | `TextComponent` 构建器（chainable methods） |
| `mobs.rs` | 生物特化数据 + `MobCast` 下转型 |
| `bedrock-packets` / `java-packets` | 底层包访问（高级用法） |
| `game-rules` / `game-events` / `biomes` / `entity-types` / `attributes` | 各 vanilla 查询面 |

---

## 十五、沙箱与日志

### 15.1 能力清单（摘自 SDK `permissions.rs`）

| 类别 | 常量 |
|---|---|
| 文件 | `FS_READ_DATA`、`FS_WRITE_DATA`（仅限插件私有数据文件夹） |
| 网络 | `NETWORK_DNS/TCP/UDP`、`NETWORK_TCP_CONNECT/BIND`、`NETWORK_UDP_CONNECT/BIND`、`NETWORK_LOOPBACK`、`NETWORK_OUTBOUND`、`HTTP_OUTBOUND` |
| 系统 | `SYS_ENV`、`SYS_ENV_PREFIX(<name>.)`、`SYS_INFO`、`SYS_INFO_CPU/RAM/OS` |

### 15.2 日志

插件内正常使用 `tracing`（`info!`/`warn!`/…）。宿主把它转发到服务器日志并自动附加来源字段：

```
INFO E2E on_load ok  plugin.target=pumpkin_e2e_plugin  plugin.module=pumpkin_e2e_plugin  src\lib.rs:95
```

---

## 十六、附录

### 16.1 WIT 接口函数数（当前 0.1.0）

```
world 252 · block-entity 172 · player 127 · display 67 · server 64 · text 37
scoreboard 28 · item-stack 28 · inventory 28 · command 27 · plugin(exports) 17
boss-bar 14 · gui 11 · datapack 9 · scheduler 7 · context 6 · services 4
messaging 4 · enchantments 4 · 其余为类型/枚举定义接口
```

### 16.2 版本与兼容策略

- `PLUGIN_API_VERSION = 3`：门控 **`PluginMetadata` 布局**（原生 dylib ABI）兼容性；WASM 组件按 WIT 契约校验。
- WIT 采用 **v0.1 直接演进**：允许破坏性变更（用户决策记录于 note/11）；新增函数对旧组件向后兼容（组件只导入其所需子集）。
- 服务端版本：`0.1.0+1.21.11-26.51`；i18n 翻译按客户端版本执行。

### 16.3 端到端验证基线

`examples/e2e-plugin` 实跑，7 个日志标记全绿即 API 链路健康：

```
E2E on_load ok
E2E service-registered-and-discovered provider=e2e-plugin
E2E channel-registered channels=["pumpkin:e2e"]
E2E config-loaded-and-merged len=56
E2E on_enable ok
E2E async-task-fired
E2E tick-event flowing (20 ticks observed)
```

### 16.4 已知边界（诚实清单）

- **9 个事件无 fire 点**（vanilla 机制缺失）：见 §4.3 与 note/11 §三。
- **自定义 AI 目标无公开注册入口**：trait 与宿主回调链路已就绪，缺公开注册函数（§十二）。
- EntityScheduler 的触发/跳过路径、join/chat 优先级实机排序需真实玩家/mob 验证（无头环境拿不到 `Entity` 资源；WIT 无世界级实体枚举/生成接口）。
- ChunkSave 事件低于插件边界（保存决策在 `pumpkin-world` 内部）。
- 实体查询只能经事件/邻域等途径获得实体资源（§11.2 边界说明）。
