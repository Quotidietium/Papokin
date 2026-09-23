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

## 八、勘误（2026-09-20 复核补记）

§四"⑫ EntityScheduler 的 e2e 边界"中"WIT 无世界级实体枚举/生成接口，`Entity` 资源只能从事件获得"的论断**有误**：`world.wit` 的 `spawn-entity`（:901）与 `get-entities`（:904）随 subtree 导入即存在，宿主实现为真（`wit/v0_1/world.rs:952,1289`）。实体绑定任务的触发/跳过路径**可以无头 e2e**：`world.spawn_entity` 生成实体 → 绑定 repeating 任务 → `entity.remove()` → 断言日志停止。覆盖复核全文见 [13-插件API覆盖复核](13-插件API覆盖复核-当前代码vs-Papo.md)。

## 九、第三轮：机制级全覆盖收官（2026-09-21）

目标：把 note/13 复核确认的最大剩余缺口（Registry/Tag 体系）与全部长尾机制一次推平，达成 Papo 插件 API 机制级全覆盖。验证标准不变：fmt/clippy 0 错误/test 全绿 **+ e2e wasm 实跑**；双版本（Java/Bedrock）完整；`PLUGIN_API_VERSION` 3→4；WIT v0.1 继续直接演进。

### 9.1 交付总览

| 批次 | 机制 | 关键实现 | 状态 |
|---|---|---|---|
| T1a | 运行时注册表（synced registry） | `server/registry.rs` `RegistryManager`：冻结窗口内 `register(domain,name,nbt)`，`custom_network_id = vanilla_count + index`（按版本缓存），known-packs 登录+配置双注入 | ✅ 7 单测 |
| T1bc | 自定义魔咒 | intern 表（leaked `&'static Enchantment`，id=43+index 即网络 id）；codec 桥 `set_custom_ids/custom_id/custom_name`；生成代码零改动；`item_stack.rs` UB 修复 | ✅ 21 使用点零跟进 |
| T1d | 自定义伤害类型 | `ResolvedDamageType::{Vanilla,Custom}` 双轨 + `DamageTypeManager`（51+index）；`damage_with_resolved_context` 新路径，50 处 vanilla 调用点零改动；`damage-by-name` 按名伤害 | ✅ 9 单测 |
| T1e | 标签叠加 | `TagManager` 名称址叠加 + 静态表合并快照（按版本预计算）；自定义条目经 `custom_network_id` 原样入标签；**Bedrock 无标签推送机制（协议 N/A）** | ✅ 16 单测 |
| T1f | 三管理器 API 面 | `server.get_{damage_type,tag,registry}_manager()`（同 enchantment-manager 模式），host 推 `Arc<Manager>` 入资源表 | ✅ |
| T3 | 事件接线 92 臂 | context.rs 全限定路径注册（45 player + 26 entity + 8 block + 5 world + 8 server）+ witch 三事件 + 4 个跨域 fire 点（sign-command-preprocess、attempt-smash-attack 否决回退、connection-close 双版本、naturally-spawn-creatures 最近非旁观玩家） | ✅ cleanup.rs 同步扩臂 |
| T4 | Merchant / Chunk / Entity 长尾 | 村民+流浪商人 offers CRUD（流浪商人补重发管线）；区块快照分页转储 + `set_chunk_forced` 唤醒沉睡 force-ticket 机制（修掉强加载区块被卸载的真 bug）；spawn-category / entity-snapshot（NBT 往返）/ brain-memory 只读 | ✅ |
| T5 | Structure / MapView / LootTable | 运行时模板缓存注册 + `place_template_with_options` mirror 参数；MapView Java 全套（含 `locked` 地图不再被地形管线覆盖的 vanilla 缺口修复）；`loot.wit` 四函数（只映射生成管线真实消费的 4 个上下文字段） | ✅ |
| T6 | CombatTracker / DragonBattle / cookie | living-entity 六只读查询；`dragon.wit` 龙战资源（Weak\<World\> 句柄 + Mutex\<DragonFight\> 调用内锁）；cookie 存储挂 PendingConnection 经 `from_pending` 移交 JavaClient，跨 login→config→play | ✅ |
| Bedrock | 地图包考据与实现 | 见 §9.2 | ✅ 金标准字节测试 |

### 9.2 Bedrock ClientboundMapItemData 考据（0x43）

**结论先行**：Papokin Bedrock = 协议 2169/2193 ↔ MC 1.26.45（`status.rs` 实证）；1.26.40 起 UpdateFlags 位域废除，各节独立 optional。权威线序取 CloudburstMC Protocol 3.0 的 `ClientboundMapItemDataSerializer_v2168`（v2169/v2193 无更新序列化器直接继承），gophertunnel 与 prismarine-data proto.yml 交叉一致：

```
map_id VarLong · dimension u8 · locked bool · origin BlockPos(VarInt×3)
tracked_entity_ids Opt<[VarLong]>   ← Geyser 恒发 [map_id]（1.19.50 必需）
scale Opt<u8> · tracked_objects Opt<[type i32LE + 两 optional 成员]>
decorations Opt<[image u8, rotation u8, x u8, y u8, label String, color i32LE]>
width/height/x_offset/y_offset Opt<VarInt> · colors Opt<[i32LE]>
```

**像素线序定论**（曾三方矛盾）：colors 元素 = ABGR int（`0xAABBGGRR`）小端 → 线上字节 R,G,B,A——Geyser `MapColor.getABGR()` + Cloudburst `writeIntLE` 双实证；gophertunnel `BEARGB` 是另一内存约定殊途同归。装饰色用 ARGB（Geyser `toARGB`，与像素布局不同，照抄）。Java 色字节→ABGR：base=id>>2（0 全透明），shade=id&3，乘 vanilla 亮度表 [180,220,255,135]/255 地板除（对 Geyser 金色条目单测断言）。

**图标映射**：Java `MapDecorationType` 0..41 → Bedrock image 0..24，照 Geyser `BedrockMapIcon`（frame→7、target_x→4 黑、banner×16→13+染料 RGB、red_x→4、trial_chambers→24，1.21.11 新增无图类型回落白标）。**Bedrock 客户端每个装饰必须配一个 tracked object**（伪 entity id=下标），否则不渲染图标。origin 发 (0,0,0)（1.19.20 必需）。装饰 x/y：Java i8 直接 `as u8`（补码重解释）。

落点：`bedrock/client/map_item_data.rs`（手写 PacketWrite——derive 不给 `Option<Vec<T>>` 写计数；两则金标准字节测试锁线序）、`world/map.rs` `bedrock_map_packet + map_color_to_abgr + bedrock_icon`、`send_to_holders`/`tick_maps` 改 `try_enqueue_packet_editioned` 双版本发送。

### 9.3 实跑暴露并修复的真 bug

1. **`get-registry-key` 违反 WIT 契约**（e2e merchant 段暴露）：host 直出 pumpkin-data 裸键（`"emerald"`），契约文档与 SDK `IntoItemKey` 均为命名空间形式（`"minecraft:emerald"`）——任何插件按文档比对注册键必然失败。修为无冒号时补 `minecraft:` 前缀。
2. **强加载区块被卸载**：force-ticket 机制早已存在但从未被调用；`World::set_chunk_forced` 接上票据同步，从源头阻止卸载。
3. **locked 地图被地形管线覆盖**：vanilla 语义缺口，`MapData::update()` 对 locked 直接返回（插件画布不被冲刷）。
4. **女巫三事件 fire 点**语义：throw 取消→无弹射物；consume 取消→效果不应用但仍消耗；ready 取消→不装备不饮用。

### 9.4 门禁与 e2e 终版（2026-09-21）

- `cargo fmt --all -- --check` 清洁；`cargo clippy --workspace --all-targets` **0 错误**；`cargo test --workspace` **1059 通过 0 失败**（39 ignored 为需真机 gametest）。
- e2e 无头实跑（`target/e2e-run`，新编服务端 + 新编 wasm）：**40 个 E2E 标记、零失败类**。机制标记全绿：`registry-summary`（三管理器 true）、`loot-generate`（7 stacks/14 items + zombie context Ok(2)）、`combat-queries`（0→1 入队、`last_damage_type=Some("generic")`）、`map-view`（roundtrip=true）、`chunk-snapshot`（24 段 98304 状态）、`structure-registered-and-queryable`/`structure-placed`、`merchant-trade-offer-builder`、`dragon-fight`（三世界路径）、`cookie-api-ready`、`brain-memory-query`、`entity-snapshot-roundtrip`、`spawn-category-mapped`。
- **e2e 无头化改造**：原 join 门控的 4 个实体/战斗标记改为 `on_enable` 内 `spawn_entity(Zombie)` 驱动（living-entity 资源挂 `damage-by-name` + 六查询）；join 路径保留玩家专属半边（player=MISC、玩家快照拒绝、evt-* 六事件、join 优先级、plugin-message）——这些仍需真实客户端，与 §四同口径。
- WIT 规模：58 个 .wit、997 函数；事件 273→368 种。

### 9.5 遗留（诚实清单）

1. `LootGenerateEvent` 事实死代码：fire 点入口零调用者，10+ 真实生成路径（箱/掉落/钓鱼…）绕过统一入口；payload（仅表 key）与生成签名（seed/上下文）不匹配，汇聚需跨模块设计，另案处理。钓鱼未接战利品表（源码 TODO）。
2. cookie：config 阶段发包路径未暴露（Player 资源 play 相位才存在）；请求-响应无事务关联（同 Paper）；无响应到达事件。
3. 脑记忆只读（无 set 面）；结构放置不含实体；区块快照不含光照/高度图；强加载票据不落盘；`damage-by-name` 事件数据尚不回传自定义伤害类型名。
4. `list-loot-tables` 需 pumpkin-data 先提供枚举 API；Bedrock 登录期 validate/whitelist 事件待协议面成熟。

## 十、基岩版支持整体移除（2026-09-21）

目标：清理项目基岩版（Bedrock）相关代码，服务器回归纯 Java 版（§9.5 第 4 条"Bedrock 登录期事件"随之作废）。验证标准不变：fmt/clippy 0 错误、test 全绿、e2e wasm 无头实跑 40 标记零失败类。版本号 `0.2.0+1.21.11-26.51` → **`0.3.0+1.21.11`**（弃版本方案中的 bedrock 段）。

### 10.1 拆除面

- **协议/网络层**：`ClientPlatform`/`DisconnectReason` 枚举删除；`Player.client` 定型 `Arc<JavaClient>`；bedrock 网络模块树（NetherNet/WebRTC 信令、OIDC 密钥拉取、server_guid、登录链校验）全删。
- **世界广播塌缩**：`*_editioned` / `*_bedrock` 双版本函数族统一为 Java 单版（`broadcast_editioned`→`broadcast_packet_all` 等）；`spawn_bedrock_player`（约 590 行）、`play_bedrock_level_sound`、`component_to_bedrock_text`、`bedrock_block_breaking_rate` 删除；`send_entity_status` 塌缩为 2 参；`BlockBreakingProgress` 摘除仅供 Bedrock 的 speed 字段。
- **命令错误塌缩**：`CommandErrorType::new` 双翻译键（java+bedrock）→ 单 java 键；241 调用点 + 28 常量定义经 tokenizer 感知 Python codemod 一次推平（尾逗号陷阱：按顶层逗号切片计数而非数逗号）；unknown-command 测试期望回归 java vanilla 文案。
- **pumpkin-data 代码生成**：bedrock_biome/bedrock_creative/wit::bedrock_packet 生成器删除；block/item/biome 生成器的 Bedrock 排放面（geyser 映射、`STATE_ID_TO_BEDROCK`、`be_network_id` 等）全删；全量重生成后 translation.rs 减 46612 行（Bedrock 翻译模块）；`serde_repr` 依赖移除。codegen 增加 stem 过滤器：`cargo run -- <stem>` 只跑匹配 .rs 生成器（wit+sdk 恒跑）。
- **WIT 契约**：forms.wit、bedrock-packets.wit 删除；player.wit 摘除 13 个 Bedrock 块（bedrock-player 资源、8 枚举、4 record、as-bedrock）；event.wit 摘除 Bedrock 表单响应事件；scoreboard.wit 摘除 bedrock-scoreboard 资源与 Bedrock 显示槽；text.wit 摘除 translate-cross。契约规模：58→**56 文件**、997→**975 函数**、事件 368→**367**（实测）。`PLUGIN_API_VERSION` **4→5**。
- **插件宿主/SDK**：host 侧 BedrockPlayer/Scoreboard 资源、generated_packets Bedrock 段、v0_1 player/server Bedrock impl 删除；SDK forms.rs、bedrock_form_response.rs 删除及 re-export 清理。
- **杂项**：区块调色板 Bedrock 序列化（`convert_be_network`/`BeNetworkSerialization` 等）删除；方块实体 `bedrock_block_actor_data` 钩子删除；serializer 的 Bedrock NBT 写入删除；CI reviewers.yml / README / NOTICE 同步去 Bedrock。

### 10.2 过程要点（复用价值）

- **大规模并行重构编排**：13 个子代理分两波按目录划界并行。第一波 7 个因供应商认证失败中途死亡留下半成品——救局手法：`git status` 盘点残局 → 以 `cargo check` 报错的 `-->` 位置限定各代理职责范围重新派发；跨目录接缝（签名漂移、调用点 arity、`address()`→字段、`closed()`→`is_closed()`）由主线在波次间统一收口（23 错→0）。
- **WIT 半再生事故**：死亡代理留下截断的代码生成器，全量 codegen 据此再生成出一批残缺 WIT（damage-types.wit 丢了被手写 server.wit 引用的资源）。恢复：`git checkout HEAD -- 契约目录` → 重删两个目标文件 → 用过滤器只跑 wit+sdk（`cargo run -- __no_such_stem__`）→ 手写 WIT 手工摘除 Bedrock 块。教训：**生成器处于半改状态时绝不跑全量再生成**。
- **e2e wasm 判别式漂移**：WIT variant 变更会移动组件判别式，checked-in `.wasm` 必须按新契约重编（32.77s）再部署 `plugins/` 与 `target/e2e-run/plugins/`，否则宿主校验即失败。
- **误判自纠**：`end_gateway` 的 `allow_bedrock` 参数险些被当恒 false 常量塌缩，grep 发现第二调用点传 true（exit-portal 搜索语义）→ 改名 `exit_portal` 保留。常量塌缩前必须数清全部调用点。

### 10.3 门禁与 e2e 终版（2026-09-21）

- `cargo fmt --all -- --check` 清洁；`cargo clippy --workspace --tests -- -Dwarnings` **debug 与 release 均 0 错误**（塌缩衍生的 17 个 redundant-clone/dead-code/stale-expect 类警告清零）。
- `cargo test --workspace`：**968 通过 / 0 失败**（较第三轮 1059 下降为 Bedrock 测试同步删除所致）。
- e2e 无头实跑（`target/e2e-run`，新编服务端 + 按新 WIT 重编 wasm）：**40 个 E2E 标记、零失败类**，机制标记与 §9.4 清单一致（registry-\* ×5、recipe-\* ×4、loot/combat/map/chunk/structure/merchant/dragon/cookie/brain/snapshot/spawn-category 全绿）。
- 磁盘纪律：构建后 `rm -rf target/debug/incremental`（AGENTS.md 新规）；本轮构建后 target 33G。

## 十一、体验对齐 Papo：机制级缺口清空（2026-09-23）

目标：所有玩家可见体验与 Papo（参考实现）一致——只要求 UX 一致，不要求实现方法一致。两轮落地后，§三「不可接线清单」从 9 个事件收敛到 4 个（全部为架构性缺口），`LootGenerateEvent` 死代码清零。

### 11.1 玩法机制补齐（第一轮）

1. **钓鱼战利品**（`entity/projectile/fishing_bobber.rs`）：收线产出真实战利品——鱼 85−海之眷顾 / 垃圾 10−2×等级 / 宝藏 5+2×等级 三表加权，宝藏需开放水域（5×4×5 空气/液体/荷叶），饵钓缩短等待，物品从浮漂飞向玩家（原版弹道），1..=6 经验，收竿损耗 1 耐久。此前钓鱼任何东西都不产出。
2. **雪傀儡**（`entity/passive/snow_golem.rs`）：寒冷群系（温度 < 0.8）脚下留雪层（受 mob_griefing 约束）；炎热群系（基础温度 ≥ 0.95 近似 snow_golem_melts 标签）每刻 1 点火焰伤害。
3. **经验瓶投掷物**（新 `entity/projectile/experience_bottle.rs`）：真实抛射实体（重力 0.03、初速 0.7），落地碎裂释放 3+rand(5)+rand(5)=3..=13 经验；此前是使用瞬间在眼前凭空生成经验球。
4. **冰霜行者**（`entity/living.rs` `tick_frost_walker`）：半径 2+附魔等级（上限 16）内仅冻结上方为空气的水源（level=0）为霜冰，60-120 刻后由霜冰计划刻自然融化。**坑：玩家是客户端权威不走 `tick_movement`，玩家与生物两条 tick 路径需各挂一次**。

### 11.2 玩法机制补齐（第二轮）与 bug 修复

5. **钟共振**（`block/entities/bell.rs`）：`raiders_hear_bell()` 实装——响铃瞬间捕获 32 格内 `#minecraft:raiders` 生物；共振 40 刻内每刻给 48 格内存活袭击者上 GLOWING 60 刻（无粒子）；`activate` 增加 world 参数。**顺带修掉隐藏 bug：共振结束 `resonate_time` 不清零导致同一口钟永远无法再次共振**。
6. **幽匿催化体蔓延**（`block/blocks/sculk/sculk_catalyst.rs`）：玩家击杀且死亡位置 8 格内有催化体时，经验被吸收（不生成经验球）并作为充能（min(xp,1000)）在 `#sculk_replaceable` 方块上催发幽匿块（上方须空气/液体，最多 64 落点）；充能 ≥10 时 1% 催发尖叫体（can_summon=false，不召唤监守者）+1% 传感器。简化说明：直接随机落点，未实现幽匿脉充能路径，玩家可见结果一致。
7. **盔甲架装备槽**（`entity/decoration/armor_stand.rs`）：手持装备右键穿入对应槽位（>1 消耗 1 个、=1 与原装备交换；创造不消耗）；空手按 主手→副手→脚→腿→胸→头 取下第一件；槽位禁用/手臂隐藏沿用 `can_use_slot`/`is_slot_disabled`；破坏与爆炸掉落全部装备；NBT 按原版 `ArmorItems`/`HandItems` 持久化；装备变化经 `send_equipment_changes` 同步客户端。
8. **粘性活塞推动方块消失**（本轮较早）：根因为活塞方块实体缺 `blockState` NBT 字段（客户端渲染/落块唯一依据）；补 `BlockState::to_state_string/from_state_string` 全量序列化（约 2.7 万状态往返测试）、tick 空气分支与缩回头部补 `NOTIFY_LISTENERS`。
9. **树下空气洞**（本轮最早，世界生成）：26.3 状态提供器注册表字符串引用是根因，codegen 静默回退 AIR；修复见世界生成管线。

### 11.3 事件接线（§三 挂账清单 9 → 4）

| 事件 | fire 点 |
|---|---|
| `SculkBloom` | 幽匿催化体每个催发点（取消则放弃该格） |
| `BellResonate` | 共振启动（取消则本次不高亮袭击者） |
| `EntityBlockForm` | 冰霜行者冻结、雪傀儡留雪（取消则不放置） |
| `ExpBottle` | 经验瓶落地碎裂（取消则不碎裂不给经验，经验数量可改写） |
| `PlayerArmorStandManipulate` | 盔甲架装备操作（取消则本次操作无效） |
| `LootGenerateEvent`（死代码清零） | `World::generate_loot(&str) -> bool` 接入四处真实生成路径：实体 `drop_loot`、箱子首次打开、运输矿车 `unpack_loot`、钓鱼收线；取消即无战利品 |

**剩余 4 个不可接线事件**（需先补底层机制，另案）：VaultDisplayItem（vault 机制整体缺失）、EntityTargetBlock（无 mob 方块目标语义）、HorseJump（客户端权威移动，服务端无跳跃时机）、ArrowBodyCountChange（无中箭计数状态）。

### 11.4 门禁（2026-09-23 实测）

- `cargo fmt --all -- --check`：清洁。
- `cargo clippy --workspace --all-targets`：**0 错误**。
- `cargo test --workspace`：**974 通过 / 0 失败**（较汉化轮 968 基线增加，为本轮新机制的回归测试）。
- 汉化约定延续：新增代码注释与用户可见文本全部简体中文。
