# 插件 API 强化实现记录 — Papo 机制级覆盖

> 完成时间：2026-09-20 · 前置：[10-插件系统对比](10-插件系统对比-Pumpkin-vs-Papo.md)
> 目标：在 MC Java 1.21.11 下，参考 Papo（Paper fork）插件系统，把 Pumpkin 插件 API 的功能差距补到**机制级全覆盖**。
> 约定：直接演进 `pumpkin:plugin@0.1.0` WIT 契约（允许破坏性变更）；`PLUGIN_API_VERSION` 2→3；验证标准 = 检查全绿 **+ 端到端 wasm 插件实跑**。
> 本笔记为实施记录；逐维对比分析见笔记 10。

## 一、交付总览

| # | 机制（Papo 对应物） | 实现位置 | 状态 |
|---|---|---|---|
| ⑴ | EventPriority 排序分发 + `ignoreCancelled`（Bukkit HandlerList/EventPriority） | `plugin/mod.rs` `fire()`：每事件按 `order_handlers()` 稳定排序（Lowest 先、Highest 后），blocking 全部先行于 non-blocking；`should_invoke()` 按 `cancelled_state()` 跳过 `ignoreCancelled` handler | ✅ 单测 4 个 |
| ⑵ | 异步任务（Bukkit async scheduler，墙钟） | `server/scheduler.rs` 重写：`schedule_async_delayed_task` / `schedule_async_repeating_task`（毫秒，tokio future；取消机制初版为 AtomicBool 标记，审计轮改为 CancellationToken 立即唤醒，见 §七） | ✅ e2e `async-task-fired` |
| ⑶ | 依赖分级（plugin.yml 的 hard-dep / soft-dep / loadbefore / provide） | `load_plugins`：`provides_map`（首提供者胜出+告警）、`load_after` 软边、`load_before` 反向折叠、硬缺失→传递性跳过（不动点循环）；topo 排序不变 | ✅ 编译+原测试 |
| ⑷ | 启用失败分级（Paper onEnable 失败 → 停用不卸载） | `spawn_plugin_initialization`：enable 失败 → 注销 handlers/commands、`is_active=false`、状态 `Disabled(...)`；仅 load 失败才整体卸载 | ✅ 编译 |
| ⑸ | ServicesManager（名字注册表 + 跨插件调用） | `plugin/mod.rs`：`service_registry`（service/plugin/priority/sequence，upsert、优先级+序号排序）；WIT `services` 接口；**发现窗口**：`is_plugin_loading`（Loading 态即可被发现，修掉 on_enable 自发现 None 的 bug） | ✅ e2e `service-registered-and-discovered` |
| ⑹ | 插件消息通道（PluginMessageListener / messenger） | WIT `messaging`：`register_incoming_channel`（`minecraft:*` 保留）、`send_plugin_message(player, channel, data)`；SCustomPayload 分发到 `dispatch_plugin_message` | ✅ e2e `channel-registered` |
| ⑺ | getConfig/saveConfig（YAML 深合并 → TOML 等价） | WIT `config` + `plugins/data/<name>/config.toml`（宿主构造路径，绕过沙箱 FS 权限）；`merge_values` 深合并（用户值胜出）；`Plugin::config_defaults()` 提供默认 | ✅ e2e `config-loaded-and-merged` |
| ⑻ | 命令冲突 fallback `<plugin>:<label>`（Bukkit namespaced registration） | `Context::apply_command_fallback_prefix`：注册冲突时检测树内 source，改名 `plugin:label`；`dispatcher.get_command_source()` | ✅ 编译 |
| ⑼ | permissions.yml 等价 | `server/permissions_file.rs`：`permissions.toml`（权限声明 default=true/false/op/op:N + children；玩家 UUID granted/denied），启动时加载 | ✅ 单测 `parses_defaults` |
| ⑩ | 世界前引导阶段（Paper bootstrap / loadOrder） | `LoadOrder::Startup|PostWorld` 元数据 + `load_plugins(server, phase)`：Startup 插件在 `Server::new` **建世界之前** 跑 on_load/on_enable；跨阶段依赖边正确裁剪；二次扫描不重复加载 | ✅ 编译 |
| ⑪ | 事件挂点补缺（Papo 280 事件深度挂点） | WIT 273 事件中 **60 个无 fire 点** → 本轮接线 **47 个**（3 并行子代理 35 + 主线 12）；其余见 §四 | ✅ 编译 + 抽样 e2e |
| ⑫ | EntityScheduler（Bukkit 实体生命周期绑定任务） | WIT `scheduler.wit` 增 `schedule-entity-delayed-task` / `schedule-entity-repeating-task`（`handler-id` + `entity-id` 约定）；`ScheduledTask.entity_id: Option<i32>`，`tick()` 触发时跨 `server.worlds` 解析 `get_entity_by_id`，实体不存在即静默跳过（repeating 永久停）；SDK 增 `EntitySchedulerExt`（`Entity::schedule_entity_{delayed,repeating}_task`） | ✅ 编译；e2e 见 §四说明 |

## 二、WIT / SDK 变更（`crates/pumpkin-plugin-wit/v0.1`、`crates/pumpkin-plugin-api/src`）

- 新增接口：`services.wit`、`messaging.wit`、`config.wit`。
- 扩展：`context.wit`（register-event +`ignore-cancelled`）、`scheduler.wit`（异步任务族 + 实体绑定任务族，见 ⑫）、`metadata.wit`（load-after/load-before/provides/load-order）、`plugin.wit`（export on-enable/on-disable/handle-plugin-message）。
- SDK 对应新增 `services.rs` / `messaging.rs` / `config.rs`，`scheduler.rs` 增 `SchedulerExt` 异步方法与 `EntitySchedulerExt` 实体绑定方法；`Plugin` trait 增 `on_enable` / `on_disable` / `on_plugin_message` / `config_defaults`。
- 事件 Payload：`Payload::cancelled_state() -> Option<bool>` 默认 None；`#[cancellable]` 宏在 `#[derive(Event)]` 前注入 `cancelled` 字段，derive 检测到该字段即自动实现 `cancelled_state`。
- **`PLUGIN_API_VERSION` 2→3**（`PluginMetadata` 布局变更 = native dylib ABI 破坏）。

## 三、事件补缺明细（⑪）

接线 47 个事件 fire 点（取消语义：cancellable 事件取消即阻止后续效果；列表见 git diff `block/`、`entity/`、`world/`、`net/`、`item/`）：

- block/world 域（含新机制补齐）：BlockFade、BlockMultiPlace（附带新增 `extra_placed_blocks` 钩子：门/床/高植物多格放置）、BlockReceiveGameEvent、BlockShearEntity、BlockDispenseArmor、BlockDispenseLoot、ChunkLoad、FluidLevelChange、MapInitialize、TNTPrime（补 5 种点火原因 + 爆炸连锁路径）、CauldronLevelChange、BellRing、ChunkSend（既有，验证）
- entity 域：CreatureSpawn（BREEDING / NATURAL / CHUNK_GENERATION 三路径）、CreeperPower、PigZap（补实现猪→僵尸猪灵转化）、PigZombieAnger、EnderDragonChangePhase、EntityCombustByBlock/ByEntity（箭矢/火焰弹/火伤附魔/引燃魔咒 5 处）、EntityKnockback/ByEntity、EntityPortalEnter/Exit、EntitySpellCast（唤魔者三法术）、HangingBreak/ByEntity、SheepDye/RegrowWool、SlimeSplit、StriderTemperature（补实现温度切换）、ItemMerge
- player 域：AsyncPlayerChat（改 format 生效）、PlayerPreLogin（代理路径除外，见下）、PlayerCommandPreprocess、PlayerHide/ShowEntity（entity_tracker 配对点）、PlayerPortal、PlayerExpCooldownChange、PlayerNameEntity、PlayerTakeLecternBook（`LecternController` 新增 `on_book_take_click` 否决点）、PlayerSwapHands（既有）

**不可接线清单（无底层机制，诚实缺口而非 API 缺口）**：

| 事件 | 原因 |
|---|---|
| SculkBloom | 幽匿催化体无蔓延逻辑（仅 on_place） |
| BellResonate | 共振分支为 TODO 死代码 |
| VaultDisplayItem | Vault 无展示物品机制 |
| EntityBlockForm | 雪傀儡无雪迹、Frost Walker 为占位 |
| EntityTargetBlock | 无 mob-方块目标语义 |
| ExpBottle | 投掷瓶不存在（即时生成经验球） |
| HorseJump | 骑乘移动客户端权威，无服务端跳跃 |
| ArrowBodyCountChange | 无中箭计数状态 |
| PlayerArmorStandManipulate | 盔甲架无装备槽实现（TODO） |

**架构性受限**：ChunkSave（保存决策在 `pumpkin-world`，低于插件边界）；EntityRemove（68 个分散移除点无 cause 汇聚点）；EntityTargetLivingEntity（目标路径无 reason 贯通）；PlayerPreLogin 代理分支（uuid 后置，由 finish_login 的 AsyncPlayerPreLoginEvent 覆盖）。

## 四、端到端验证（用户选定的验证标准）

测试插件：`examples/e2e-plugin/`（**非 workspace** crate，path 依赖 `pumpkin-plugin-api`，`wasm32-wasip2` 直接产出组件 0x1000d）。

```
cargo build --target wasm32-wasip2   # 在 examples/e2e-plugin 下
```

复制到 `plugins/` 后实跑服务器，7 个断言标记全绿：

```
E2E on_load ok
E2E service-registered-and-discovered provider=e2e-plugin   ← 发现窗口修复后转绿
E2E channel-registered channels=["pumpkin:e2e"]
E2E config-loaded-and-merged len=56
E2E on_enable ok
E2E async-task-fired                                        ← 墙钟异步任务
E2E tick-event flowing (20 ticks observed)                  ← 事件分发链路
```

另有单测：`bukkit_priority_order`（Lowest→Highest 稳定序）、`ignore_cancelled_gate`、`cancellable_event_reports_cancellation`、`parses_defaults`、config 深合并 ×4。

**⑫ EntityScheduler 的 e2e 边界（诚实说明）**：WIT 无世界级实体枚举/生成接口，`Entity` 资源只能从事件获得 → 无头 e2e 无法拿到实体，实体绑定任务的**触发/跳过路径无法无头实跑**（与 join/chat 优先级同类，需真实玩家/mob）。已验证的部分：e2e 插件对新 SDK 的编译兼容（wit-bindgen 按 scheduler.wit 再生成 guest 绑定后重编通过）；宿主语义在 `server/scheduler.rs` `tick()` 的 4 行存活门控（`worlds.load().iter().any(get_entity_by_id)`，缺席即 `continue`，repeating 因此不再重排）+ `wit/v0_1/scheduler.rs` 委托层，代码可审。接玩家后验证法：插件在 `EntityDamageByEntity` 等事件里对实体 `schedule_entity_repeating_task`，实体退场后日志应停止。

## 五、验证结果（2026-09-20，EntityScheduler ⑫ 收尾后终版）

- MC 版本：workspace `0.1.0+1.21.11-26.51`（Cargo.toml），版本门控纪律未破（历史 `if *version <` 门控零改动；remap 以 `NATIVE_DATA_VERSION` 为准）
- `cargo fmt --all`：清洁
- `cargo clippy --workspace --all-targets`：**0 错误**（含首次全量 lint 后修掉的 9 处：hanging_entity 冗余 clone、login_start 过长[提取 `handle_login_start_direct`]、ServiceRegistration 文档段、`is_plugin_loading`、`fire` 迭代、wasm_host identity map、chunk_data VarInt/checked_div、pumpkin-world 两处 clippy 1.98 误报 const-thread_local [已加定向 allow]）
- `cargo test --workspace`：**全绿**（本次干净缓存实跑：117+78+230+5 通过、0 失败；ignored 为需真机的 gametest）
- e2e 实跑：7 标记在当前二进制 + 新编 wasm 下全绿（`target/e2e-run`，见 §四）
- 缓存清理（同日）：root debug target 76.7GiB（孤儿产物为主）+ e2e crate target 1.3GiB 已清，释放 78GiB；保留 `target/release`（当日 dist 打包缓存）、`dist/` 成品、`target/e2e-run` 运行记录；e2e 成品 wasm 备份于 `plugins/`

## 六、遗留与建议（未在本轮范围）

1. **join/chat 优先级的实机验证**需真实客户端连接（e2e 插件已注册 Lowest/Highest 双 handler，加入玩家后看日志顺序即可）。
2. 不可接线清单里的 9 个事件，随对应 vanilla 机制补齐后按同模式加 fire 点即可（WIT/Payload/分发均已就绪）。
3. `EntityRemoveEvent` 若要落地，建议先在实体移除路径统一收敛一个带 cause 的内部入口。
4. PlayerPreLogin 代理分支：待 Velocity/Vine 阶段可拿到 uuid 后补一发同步事件（可选）。

## 七、第二轮：代码审计与稳定性修订（2026-09-20，同日稍后）

补强交付后对插件系统做了两轮全量代码审计（稳定性 / 安全性 / 长跑健壮性），运行时执行器、加载器、资源管理深读无缺陷；以下为修复清单，插件可见行为变化已同步进笔记 12 对应章节。

**第一波（调度器 / SDK 资源生命周期 / guest 可控面）**

1. **异步任务取消即时化**：AtomicBool 轮询标记 → `CancellationToken`（`AsyncTaskEntry { cancel, plugin: Weak<WasmPlugin> }`）。修复前 guest 睡 1 小时，disable/cancel 只能等睡眠自然结束、store 内存一直被占；现在立即唤醒释放。`cancelled_tasks` 集合在 disable 时顺带清空（防长跑泄漏）。
2. **`wait_for_plugin` / `wait_for_all_plugins` 丢失唤醒修复**：`Notified::enable()` 先于状态重读，消除通知到达早于 await 的悬挂窗口。
3. **AiGoal 公开注册 + 宿主接线**：SDK 增 `AiGoalManager::register`（GeneratorManager 同款模式）；宿主 `CustomWasmGoal` 的 `can_start`/`should_continue` 此前**硬编码 false**（自定义 AI 目标功能整体处于死亡状态），已接线到 guest 导出，回调失败一律按 false 处理。
4. **SDK 任务处理器表泄漏**：`TASK_HANDLERS` 只增不减 → 一次性任务触发即移除、`cancel_task` 连带清理；实体任务被宿主静默终止时无 WIT 回调，残留项为已知边界（笔记 12 §16.4）。
5. **guest 可触发的 panic 收敛**：两处 `send_packet` 宿主函数的包序列化（宏生成代码）包 `catch_unwind`，畸形包不再能打崩宿主任务。
6. **Event derive 跨模块同名防护**：`get_name` 改用 `std::any::type_name`（含模块路径），杜绝不同模块同名事件类型的向下转型混淆。

**第二波（加载器 / 运维面）**

7. **热重载去抖**：notify 一次保存产生连串事件，原先每个事件都触发完整 unload + JIT 重载（大插件单次 ~155s）并堵死 notify 线程；改为 500ms 静默窗口按路径去重合并，扩展名判断大小写不敏感。
8. **JIT 缓存写失败降级**：`load_component` 的 `.cwasm` 缓存写失败（并发加载者持锁等）从致命错误降为告警——编译产物已在内存，缓存只是加速。
9. **config.toml 原子写**：`load_config` 合并回写与 `save_config` 统一走 `write_config_atomic`（tmp + rename），崩溃不留截断配置。

**门禁**（两波收尾均复跑）：`cargo fmt` 清洁；`cargo clippy --workspace --all-targets` 0 错误；`cargo test --workspace` 992 通过 / 0 失败；e2e 实跑 7 标记全绿 ×2（第二次覆盖了新的原子配置写路径）。
