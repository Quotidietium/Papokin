# 13 · 插件 API 覆盖复核：当前代码 vs Papo（REF）

> 生成时间：2026-09-20 · 锚定 **master @ b6af9b3c7**（13 个提交未推送，全部为当日插件轮）
> 方法：Papokin 侧**全量实测当前代码**（WIT 逐文件函数计数、fire 点 grep、宿主实现抽查、配置/调度器/沙箱读码）；Papo 侧两路并行重扫 `REF/Papo-Java-0.80.0-src/paper-api/`（org.bukkit 全树 1268 文件 + io.papermc.paper 全域）
> 与既有笔记的关系：[10](10-插件系统对比-Pumpkin-vs-Papo.md) 是**机制对比快照**（锚定 3eee993d1，成文当日覆盖工程才落地）；[11](11-插件API强化实现记录-Papo机制级覆盖.md) 是实施记录；[12](12-插件API文档.md) 是开发者活文档。**本文是覆盖度复核**：以当前代码为准，回答两个问题——①笔记 10/11 声称的覆盖现在是否全部属实（逐项验证）；②把镜头从"12 项机制"拉宽到 Papo 全部插件 API 面后，**剩余缺口**是什么。

## 一、一句话结论

笔记 11 的 12 项机制级覆盖**在当前代码中逐项验证全部属实**（§三）；把镜头拉宽到 Papo 全 API 面（org.bukkit 1268 文件 + Paper 扩展）后，**机制层面无新增缺口**，剩余差距集中在 **9 个具体 API 面**（最大的是通用 Registry/Tag 注册修改体系），以及事件数量 273 vs ~395。复核中发现笔记 11/12 一处事实性漂移：`world.spawn-entity` / `world.get-entities` **存在且有完整宿主实现**，"WIT 无世界级实体枚举/生成接口"的论断不成立——EntityScheduler 的无头 e2e 验证实际上可行（§七.2）。

## 二、API 面总量（实测当前代码）

| 指标 | Papokin（当前） | Papo（0.80.0） |
|---|---|---|
| 契约/API 文件 | 51 个 WIT（`crates/pumpkin-plugin-wit/v0.1`） | org.bukkit 1268 个 .java + io.papermc.paper 扩展 |
| 函数总数 | **896**（`grep -c ': func('`，含 resource 方法与 17 个 guest export） | 不可比（方法级数千） |
| 事件类型 | **273**（event.wit variant 实测） | ~**395**（org.bukkit.event 304 文件约 296 个具体事件 + io.papermc.paper.event 99 个 Paper-only） |
| 事件挂点 | **288 个 fire 调用点**（业务代码，另有 5 处 `send_cancellable!`、2 处 `has_handlers` 零监听器门控） | 遍布 vanilla 的 callEvent + ~40 个零监听器门控补丁 |
| 最大接口 | world 252 · block-entity 172 · player 119 · server 64 · display 62 | entity 包 219 文件 · block.data 123 文件 |
| 调度器 | 7 函数（tick×2 + 异步墙钟×2 + 实体绑定×2 + cancel） | BukkitScheduler + Folia 四调度器（Papo 为 fallback 实现） |
| 沙箱能力权限 | 15 个能力常量（SDK permissions.rs），WASI 层强制 | 无此维度 |
| API 版本 | `PLUGIN_API_VERSION = 3`（`plugin/mod.rs:37`），WIT v0.1 直接演进 | api-version 门控 + PluginRemapper |

WIT 逐文件函数数（实测，`grep -c ': func('`，与笔记 12 §16.1 的旧数字有出入处以本表为准——本表机械可复现）：

```
world 252 · block-entity 172 · player 119 · server 64 · display 62
text 28 · scoreboard 28 · item-stack 28 · inventory 28 · command 25
plugin(exports) 17 · boss-bar 14 · gui 11 · datapack 9 · scheduler 7
context 6 · services 4 · messaging 4 · enchantments 4 · recipe 3 · uuid 3
log 2 · i18n 2 · config 2 · metadata 1 · ipc 1
其余 23 个为纯类型/枚举接口（event/entity/particles/sounds/potions/…）
```

## 三、笔记 10 借鉴清单 6 项 + 11 的 12 机制：当前代码逐项验证

| 机制 | 当前代码证据（行号为 b6af9b3c7 实测） | 状态 |
|---|---|---|
| ⑴ EventPriority 排序 + ignoreCancelled | `plugin/mod.rs:104` `order_handlers`（Reverse 稳定排序，Lowest 先）；`:113` `should_invoke`；`:1640` `fire()` 两阶段、每 handler 前重读取消标志（:1665-1670）；单测 `bukkit_priority_order`/`ignore_cancelled_gate`（:1983-2018） | ✅ 属实 |
| ⑵ 异步任务（墙钟） | `server/scheduler.rs:244,297`（async delayed/repeating）；取消为 `CancellationToken` **即时唤醒**（:365-381，审计轮改）；`finish_async_task` :357 | ✅ 属实 |
| ⑶ 依赖分级 | `plugin/mod.rs:952-1041`：`provides_map` 首提供者胜出（:959-975）、`load_after` 软边（:1010-1015）、`load_before` 反向折叠（:1021-1041）、硬缺失→传递性跳过不动点（:1044-1063） | ✅ 属实 |
| ⑷ 启用失败分级 | `plugin/mod.rs:696-761`：enable 失败 → 注销 handler/命令 + 补调 `on_disable`（Paper parity,:739）+ `is_active=false` + `Disabled` 态；仅 load 失败整体卸载（:763-804） | ✅ 属实 |
| ⑸ ServicesManager | `plugin/mod.rs:1747-1811`（priority + sequence tie-break，降序排序）；发现窗口：`is_plugin_loading`（:1396-1400）使 Loading 态可被发现（:1799-1802） | ✅ 属实 |
| ⑹ 插件消息通道 | `plugin/mod.rs:1817-1920`：`minecraft:*` 保留（:1818-1820）、`dispatch_plugin_message` 按注册序分发到 active 插件 | ✅ 属实 |
| ⑺ getConfig 等价 | WIT `config.wit`（load-config 深合并/save-config）；宿主 `wit/v0_1/config.rs`；**当前为原子写**（tmp+rename，审计轮 commit dae6b3b22） | ✅ 属实 |
| ⑻ 命令 fallback 前缀 | `plugin/api/context.rs:247-267` `apply_command_fallback_prefix`（冲突改名 `plugin:label`）；权限命名空间强制 `:192-205`（审计轮收紧：外插件命名空间直接拒注册） | ✅ 属实 |
| ⑼ permissions.toml | `server/permissions_file.rs`（启动加载，default=true/false/op/op:N + children + 按 UUID 授予/拒绝） | ✅ 属实 |
| ⑽ Startup 引导相位 | `load_plugins(server, phase)`（`plugin/mod.rs:816`）；相位裁剪 :928-950；跨相位依赖边裁剪 :1073-1079 | ✅ 属实 |
| ⑪ 事件补缺 47 处 | 当前业务代码 **288 个 fire 调用点**（grep 实测）；笔记 11 的 9 个不可接线事件 + 3 个架构受限事件，当前业务引用仍为 **0**（逐事件 grep 验证，清单仍成立） | ✅ 属实 |
| ⑫ EntityScheduler | `server/scheduler.rs:175,209`（entity delayed/repeating）；存活门控 :490-498（实体缺席 `continue`，repeating 永停）；SDK `EntitySchedulerExt` | ✅ 属实（验证手段见 §七.2 修正） |

**结论：笔记 11 §五的门禁声称（fmt/clippy 0 错误/992 测试/e2e 7 标记）与机制清单，在当前 HEAD 上未发现回退。**

> **2026-09-23 批注**：§三 ⑪ 行「9 个不可接线事件当前业务引用仍为 0」已过时：SculkBloom、BellResonate、EntityBlockForm、ExpBottle、PlayerArmorStandManipulate 五个事件已随对应机制（幽匿催发蔓延、钟共振、冰霜行者/雪傀儡留痕、经验瓶投掷物、盔甲架装备槽）接线 fire 点，`LootGenerateEvent` 死代码亦已汇聚四处真实生成路径。剩余 VaultDisplayItem、EntityTargetBlock、HorseJump、ArrowBodyCountChange 仍为架构性缺口。详见 note/11 §十一。

## 四、子系统覆盖矩阵（镜头拉宽到 Papo 全 API 面）

图例：✅ 覆盖（含等价不同形） · ◐ 部分覆盖 · ❌ Papo 有而 Papokin 无 · ★ Papokin 独有

| Papo 子系统（规模证据） | Papokin 等价物（当前证据） | 状态 |
|---|---|---|
| 事件（~395 个；org.bukkit.event 304 文件 + paper.event 99） | 273 事件 + 288 fire 点；优先级/ignoreCancelled/取消写回/两阶段只读档 | ◐ 机制全覆盖，数量与挂点深度仍少（Paper-only 事件族如 connection configuration、track/untrack entity、ServerResourcesReloaded、WhitelistStateUpdate、ClientTickEnd 未覆盖） |
| 命令（command 26 文件 + paper brigadier 35 文件：registrar/CustomArgumentType） | Brigadier 克隆 + `register_command`（context.rs:216）+ 40+ ArgumentType + 补全 + fallback 前缀 + 解析期权限过滤 | ✅ |
| 调度器（BukkitScheduler + Folia 四件套 fallback） | scheduler.wit 7 函数：tick/异步墙钟/实体绑定；cancel 即时 | ✅（RegionScheduler 在 Papo 也是主线程 fallback，不算差距） |
| 权限（permissions 9 文件 + Paper PermissionManager） | 双层模型：沙箱能力权限（独有）+ 玩家节点树 + permissions.toml + 命名空间强制 | ✅+ |
| 配置（configuration 21 文件 YAML 体系 + ConfigurationSerializable） | config.wit 深合并 + 原子写 + 宿主代管 | ✅（序列化框架无对应，TOML 直写替代） |
| 服务（SimpleServicesManager） | service_registry + Loading 发现窗口 + IPC 调用 | ✅ |
| 插件消息（messaging 11 文件 Messenger） | messaging.wit 4 函数 + SCustomPayload 分发 | ✅ |
| IPC | — | ★ 独有（同步请求/响应 + 重入链防死锁） |
| PDC（persistence 7 文件） | `PersistentDataHolder`：实体/玩家/方块实体/区块/世界/物品栈，11 种类型化方法（SDK persistent_data.rs 477 行） | ✅ |
| BlockData（block.data 123 文件 typed 子接口） | 通用属性 API：`resolve-block-state`/`get-block-properties`/`get-states-for-block` 等（world.wit 尾部 ~25 个查询函数） | ✅ 等价不同形（泛型属性 vs 123 个 typed 接口） |
| 方块实体（block 62 文件 ~60 typed state） | block-entity.wit 172 函数，30+ 具体方块实体资源（箱/炉/讲台/告示牌双面 front/back :73-76…）+ 裸 NBT 读写 | ✅ |
| 实体（entity 219 文件全类型层级） | world.wit entity/living-entity/mob 三资源 ~120 函数：装备/属性修饰/vehicle/passengers/raycast/**寻路**（navigate-to-pos/entity、pathfinding-malus、look-at） | ✅ 主体覆盖；◐ EntitySnapshot/Brain MemoryKey/SpawnCategory 无 |
| 寻路（Paper Pathfinder/MobGoals，com.destroystokyo 系） | mob 资源 navigate-* 全家 + set-pathfinding-malus | ✅ |
| AI 目标自定义 | AiGoalManager + 5 个 guest 导出（宿主 can_start/should_continue 已接线，审计轮修复前为死功能） | ★ 独有（Bukkit 需 NMS hack） |
| 玩家（Player 接口 ~千方法级） | player.wit 119 函数 + 表单/对话框/末影箱/冷却/队伍扩展 | ◐ 主面覆盖，长尾缺失（见 §五） |
| 离线玩家（OfflinePlayer 全量：背包/位置/统计/上次登录） | 仅 op/ban/whitelist 离线操作（server.wit:32-111）+ platform-offline-id | ◐ 无离线背包/位置/数据 API |
| 物品/ItemMeta（inventory 66 + meta 41 文件） | item-stack 28 + data-components（1.21 组件面）+ ItemAttributeModifier | ✅ 等价不同形（组件 API 即 1.21 现代形态） |
| 配方（Recipe 全族注册含 SmithingTransform/Stonecutting/Complex） | recipe.wit register-shaped/shapeless/cooking + SDK 1097 行构建器 | ◐ **无 smithing/stonecutting 注册** |
| 附魔（enchantments + RegistryEvents 注册） | enchantments.wit register-enchantment + manager + SDK EnchantmentBuilder | ✅ |
| 数据包（paper DatapackManager + **DatapackRegistrar 插件注册包**） | datapack.wit 9 函数：list/enable/disable/reload/execute-function | ◐ 管理有，**插件注册 datapack 无** |
| 世界（World/WorldCreator/RegionAccessor） | server.wit create/unload/save_all + world.wit time/weather/border（set-diameter 带 speed）/gamerule/spawn | ✅ |
| 区块（Chunk/ChunkSnapshot/persistence flags/getChunkAtAsync） | world.get-chunk + chunk 自定义数据 + ChunkLoad/Send 事件 | ◐ 无快照/异步加载/持久标记 API |
| 世界生成（generator 7 文件：ChunkGenerator 整体替换 + BiomeProvider + LimitedRegion） | worldgen 4 阶段挂钩（biomes/noise/surface/features）+ GeneratorManager + set-chunk-generator | ✅+（嵌阶段为独有；BiomeProvider/LimitedRegion 无） |
| 地图（map 9 文件 MapView/MapRenderer/MapCanvas） | 仅 map-block-entity 资源 + map_initialize 事件 + map-color | ❌ 无自定义渲染管线 |
| 结构模板（structure 4 文件 StructureManager/Palette 读写放置） | 仅 async_structure_generate/spawn 事件 | ❌ |
| 战利品表（loot 5 文件 LootTable 查询/fill/LootContext） | 仅 loot_generate 事件 | ❌ 无查询/fill API |
| 村民交易（Merchant/MerchantRecipe/MerchantView） | screens.wit 有 merchant 类型 + trade_select 事件 | ❌ 无自定义交易编辑 |
| 酿造（PotionBrewer + Paper PotionMix/SuspiciousEffectEntry） | potions.wit 类型查询 | ❌ 无自定义酿造注册 |
| 计分板数字格式（paper scoreboard/numbers，1.20.3+） | scoreboard.wit:10 `number-format` variant + add/update-objective 参数 | ✅ |
| BossBar（boss 7 文件含 KeyedBossBar/DragonBattle） | boss-bar.wit 14 函数 | ✅；◐ DragonBattle（respawn 序列/gateway）无，仅 phase 事件 |
| 进度（advancement 7 文件） | advancement 查询 + 进度授予/撤销 + server.wit get-advancement | ✅ |
| 传送标志（paper TeleportFlag 相对/状态保持） | entity teleport(pos, world) 绝对传送 | ◐ 无相对 flags |
| 对话 UI（paper dialog 3 文件 + registry/data/dialog 全树 + PlayerCustomClickEvent） | java-dialogs.wit 全类型树 + player.show-dialog/clear-dialog + dialog 事件域 | ✅ |
| Bedrock 表单 | forms.wit + player.open-form + bedrock_form_response 事件 | ★ 独有（Papo 无 Bedrock） |
| 包级访问 | PacketReceived/PacketSent 可取消事件 + java-packets/bedrock-packets 裸包 | ★ 独有（Papo 仅 4 个 packet 事件，无裸包写） |
| 客户端 cookie（paper connection cookie 读/写，login/config/game 三阶段） | 无（仅裸包层可达） | ❌ |
| Registry/Tag 体系（paper registry 78 文件：RegistryAccess/RegistryEvents compose/add/WritableRegistry/tag/set/holder；TAGS registrar） | 按域只读枚举接口（biomes/entity-types/attributes/damage-types…）+ enchantment/recipe 两个注册口 | ❌ **最大剩余缺口**：自定义 wolf/cat 变体、jukebox song、instrument、banner pattern、damage type 注册与 tag 修改均无 |
| 踢速率控制（ServerTickManager rate/freeze/step） | server.wit 仅 get-mspt/get-tps 只读 | ❌ |
| 战斗履历（paper CombatTracker/CombatEntry） | damage 事件族 | ❌ |
| 元数据（metadata 10 文件 Metadatable 临时键值） | 无等价（PDC 是持久化版；临时态可用插件自身 map） | ◐ 范式差异，实际影响小 |
| 简介/皮肤（profile 3 文件 PlayerProfile/PlayerTextures） | 命令参数 game-profile + player 身份字段 | ◐ 无 textures 读 API |
| 帮助/会话（help 7 + conversations 23 文件） | 无 | ❌ 小众 |
| 构建信息（ServerBuildInfo，Papo 增 papoVersion） | 无插件可见构建信息 | ❌ 小 |
| 类共享互操作 / Maven 运行时依赖 / PluginRemapper / CraftLegacy | 结构性不存在（沙箱 + 编译期固化 + 无历史包袱） | ➖ 范式差异非缺口 |
| 沙箱能力授权 / 签名 / 市场元数据 / 逐插件热重载 / 自定义加载器 / 命令树原子换树 / 零监听器零成本 | — | ★ 独有（笔记 10 §12 已述，当前全部保持） |

## 五、剩余缺口（按建议价值排序）

1. **通用 Registry/Tag 注册修改体系**（Paper registry 78 文件）：`RegistryEvents` compose/add 相位 + `WritableRegistry` + TAGS registrar。这是 1.21 时代 Paper 插件定制内容（自定义附魔以外的注册物：wolf/cat 变体、jukebox song、instrument、banner pattern、damage type、dialog）的**主入口**，也是唯一能同时喂饱"注册"与"标签"两类需求的机制。Pumpkin 已有 LoadOrder::Startup 相位和 enchantment/recipe 两个单域注册口，加一套 `registry` WIT 接口（Startup 相位限定）是顺接架构的。
2. **Paper-only 事件族补挂**：connection configuration、entity track/untrack、ServerResourcesReloaded、WhitelistStateUpdate、ClientTickEnd 等（99 个 Paper-only 事件中挑有业务价值的）；数量差距 273→395 主要靠这块收窄。
3. **Recipe 注册补 smithing/stonecutting**（recipe.wit 目前 3 个 register 函数）。
4. **OfflinePlayer 数据读 API**（背包/位置/统计/上次登录）——运营类插件刚需。
5. **LootTable 查询/fill**、**Merchant 自定义交易**、**MapView 渲染**、**StructureManager 模板**——四个独立面，各自服务一类插件（小游戏/商店/地图画/建筑）。
6. **插件注册 datapack**（DatapackRegistrar 等价）——与 1 互补，把 datapack 从"管理"扩到"生产"。
7. 小项：teleport 相对 flags、客户端 cookie、CombatTracker、ServerTickManager、PlayerProfile textures、ServerBuildInfo。
8. 不做：Conversations/HelpMap（小众）、Maven 运行时依赖/类共享/Remapper（范式不适用）、Folia RegionScheduler（Papo 自己也是 fallback）。

## 六、Pumpkin 独有能力（本次复核确认全部保持）

AI 目标自定义注册（host 接线已活）· 区块生成阶段挂钩 · 包级可取消事件 + 双版本裸包访问 · Bedrock 表单 + Java 对话框同契约 · 逐插件热重载（500ms 去抖）· 自定义插件加载器 · 能力沙箱 15 权限 + 逐插件 override · ed25519 签名 + 市场元数据 · 插件间 IPC · 命令树 ArcSwap 原子换树全体重发 · non-blocking 结构性只读档 · 零监听器一行早退。

## 七、与笔记 10/11/12 快照的差异

### 7.1 审计轮行为变化（笔记 10 成文时点 → 当前，13 个未推送提交）

笔记 10 是快照，以下当前行为与它描述不同（均为当日修复，笔记 12 已同步）：

- 事件分发：笔记 10 时点 `get_priority()` 无调用点、按注册序执行；当前 `order_handlers` 稳定排序参与每次 fire（`plugin/mod.rs:104,1658`）。
- 异步任务取消：AtomicBool 轮询 → `CancellationToken` 即时唤醒（scheduler.rs:365-381,421-434）。
- 热重载：连串 notify 事件每事件全量重载 → 500ms 静默窗口去抖按路径合并（mod.rs:386-461）。
- JIT 编译：tokio worker 内编译 → `spawn_blocking` offload（wasm_host/mod.rs:211-220）；`.cwasm` 缓存写失败从致命降为告警（:313-324）。
- 配置/缓存落盘：`write` → tmp+rename 原子写（commit dae6b3b22）。
- 签名：校验不再回落到全零密钥（commit 7ce9310c2）。
- 包序列化：guest 畸形包触发的 panic 被 `catch_unwind` 收敛（commit b18c49967）。
- 命令权限：命名空间强制 + 裸名自动补前缀（context.rs:192-205）。
- AiGoal：`can_start`/`should_continue` 从硬编码 false 接线到 guest 导出（commit 1ec4f1f7a）。
- 事件判型：`get_name` 改用全路径 `type_name`（commit 2c81e776f）。
- `wait_for_plugin` 丢失唤醒修复（mod.rs:1350-1375）。

### 7.2 复核发现的笔记事实漂移（已就地修正笔记 12，11 追加勘误）

- **`world.spawn-entity` / `world.get-entities` 存在且为真实宿主实现**（world.wit:901,904；宿主 `wit/v0_1/world.rs:952` 枚举玩家+实体、`:1289` 经 `from_type` 构造并 `world.spawn_entity`）。笔记 11 §四/12 §11.2/§16.4 的"WIT 无世界级实体枚举/生成接口，Entity 资源只能从事件获得"**不成立**（这两函数早于插件笔记、随 subtree 导入即存在）。直接后果：**EntityScheduler 的触发/跳过路径可以无头 e2e**——`server.get_all_worlds() → world.spawn_entity(zombie, pos) → entity.schedule_entity_repeating_task(...) → entity.remove() → 观察日志停止`。
- 笔记 12 §16.1 的接口函数数（player 127/display 67/text 37/command 27）与可复现计数不符，已更新为 §二 的实测值（计数口径：`grep -c ': func('`，含 resource 方法）。

## 八、建议下一步

1. 用 §七.2 的路径补 EntityScheduler 无头 e2e（spawn → 绑定任务 → remove → 断言停止），把最后一个"需真机"标记转为自动化。
2. §五.1 的 Registry/Tag 体系若立项，先做只读 `RegistryAccess` + Startup 相位 `WritableRegistry` 两个接口，enchantment 现有注册口迁移其上。
3. join/chat 优先级实机验证（笔记 11 §六.1）仍待真实客户端；与 1 一起构成下一轮插件轮的收尾清单。
