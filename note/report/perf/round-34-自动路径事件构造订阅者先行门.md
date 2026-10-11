# 轮次 34 · 自动服务器路径事件构造的订阅者先行门（通病系统性清扫·第一梯队）

## 背景：「事件构造先于订阅者检查」通病的全仓量化

轮次 32（装备事件）与轮次 33（统计递增事件）先后确认同一通病：
调用方在检查订阅者之前就构造事件，`fire_blocking` 内部短路
（`has_handlers` 在 `block_on` 之前）救不回调用方已付的分配。
本轮对该通病做全仓量化清扫：

- 函数级重扫（以 fire 点所在函数为界，排除已带
  `has_handlers` 门的）：**83 个无门且构造含 String 的
  fire 点**（`.to_string()`/`format!(` 恒堆分配），另有 99 个
  clone 上下文点（`ItemStack` 带组件克隆分配、`Arc` 克隆不算，
  需逐点甄别）；
- 已在册的既有门（排除项）：`tick_effects` 的
  `EntityEffectTickEvent`（每刻每活跃效果，注释明示「极热路径」）、
  轮 32 装备事件、轮 33 统计递增、`PlayerChunkUnloadEvent`——
  证实该模式已是仓库既定范式，本轮是扩展而非新创。

### 热度分级与范围

- **第一梯队（本轮收编，19 处/9 文件）**：逐 tick 与高频自动
  路径——刷怪、掉落、容器吞吐、烧炼、效果施加；
- **第二梯队（归档后续）**：玩家中低频交互（合成簇
  CraftItem/PrepareItemCraft、net/java 包处理器 21 处、
  advancement）与低频面（command 2、mob 行为 7、unleash、
  lightning、dragon、piston/tnt/note/dispenser 等）——完整
  清单见函数级扫描输出，后续轮次按需收编。

## 改动（第一梯队 19 处）

| 路径 | 文件 | 事件 | 每次白付 |
|---|---|---|---|
| 自然刷怪尝试/成功 | natural_spawner.rs ×4 | PreCreatureSpawn / CreatureSpawn | 2+2 String |
| 区块生成刷怪尝试/成功 | natural_spawner.rs ×2（同上文件） | 同上 | 2+2 String |
| 实体生成（掉落/投射物总入口） | world/mod.rs | EntitySpawnEvent | 1 String |
| 物品掉落（普通/带速度） | world/mod.rs ×2 | ItemSpawnEvent | 1 String |
| 战利品生成（死亡掉落/容器） | world/mod.rs | LootGenerateEvent | 1 String |
| 漏斗吸入（实体→漏斗） | hopper.rs | InventoryPickupItemEvent | 1 String（含锁外预算的 registry_key） |
| 漏斗吐出（每 8 刻每槽） | hopper.rs | InventoryMoveItemEvent | 1 String/槽 |
| 熔炉系宏（炉/高炉/烟熏炉） | furnace_like ×3 | Burn/StartSmelt/Smelt | 1/1/2 String |
| 刷怪笼生成轮 | mob_spawner.rs | PreSpawnerSpawnEvent | 1 format! String |
| 信标效果（每 80 刻×玩家×效果） | beacon.rs ×2 | BeaconEffectEvent | 1 String |
| 实体拾取（农场高频） | living.rs | EntityPickupItemEvent | 1 String |
| 效果施加（药水/信标/食物） | living.rs | EntityPotionEffectEvent | 1 String |
| 姿态切换（潜行/游泳/滑翔） | entity/mod.rs | EntityPoseChangeEvent | 1 String |

统一范式：`if let Some(server) = world.server.upgrade() && server
.plugin_manager.has_handlers::<E>()` 先行，无监听时零构造；
fire 后字段回读收进有监听分支，无监听时按构造值等价续行。

### 逐点语义核对（红线：行为/UX 零变化）

- 门谓词与 `fire_blocking` 内部短路完全同源（同为
  `handlers.has_handlers(E::get_name_static())`）——无监听时
  旧路径构造的事件也必被丢弃、`cancelled=false`，新旧路径
  续行值逐位一致；
- **fire 后回读字段的三种形态**：
  1. 只读 `cancelled`（绝大多数）——无监听恒 false，等价；
  2. 回读事件携带值（FurnaceBurnEvent.burn_time、
     FurnaceStartSmeltEvent.cooking_time）——已核对构造器
     `const fn new` 将字段初始化为传入值，无监听时跳过构造
     需按构造值等价续行：burn 的 `burn_ticks` 初值即
     `adjusted_fuel_ticks`（天然等价）；start_smelt 的无监听
     分支显式回写 `recipe_cook_time.min(u16::MAX) as u16`
     （与事件值路径同钳制——注意不能复用无服务器分支的
     `as u16` 截断写法，那对数据包超大值是回绕不是钳制）；
  3. `is_some_and` 复合（信标）——门以 `has_handlers() &&
     { 构造; fire; cancelled }` 短路，无监听恒 false，等价；
- 漏斗吐出把 server 升级+门提升到槽循环外（每周期一次），
  每槽 cancelled 语义不变；漏斗吸入把 `registry_key.to_string()`
  连同第二次锁获取一并移入门内（无监听时只做 is_empty 检查）；
- 事件构造器全部为纯 `const fn new`（字段直存），无副作用；
  wasm 宿主侧事件管道按订阅工作，不受调用方门影响。

## 双臂基准

`benchmark/src/bin/auto_path_event_gate.rs`（第 34 个 bin）。
「自动服务器运转」混合轮模型（无订阅者）：每轮复刻真实构造
形态——PreCreatureSpawn（format!+to_string）、CreatureSpawn
（×2）、EntitySpawn、ItemSpawn ×2、漏斗 4 槽、每 8 轮熔炉
周期（burn+smelt×2），共 11 String/轮保底。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 两臂语义指纹一致 | PASS（0xa40afcad713aa1e5） |
| G2 | 旧臂 ≥ 轮数×11、新臂 == 0 | PASS（1 137 500 ≥ 1 100 000；0） |
| G3 | 新臂耗时 ≤ 旧臂 | PASS（0.14 ms ≤ 32.31 ms） |
| G4 | 旧臂毛字节 > 新臂 | PASS（16.94 MB > 0） |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 混合轮 ×100 000 | 1 137 500 allocs / 16.94 MB / 32.31 ms | **0** / **0** B / 0.14 ms | 分配 −100%、耗时 **225.5×** |

旧臂 11.375 String/轮 = 11 保底 + format! 超容量增长分配
（轮次 33 已实证该 1.6 系数形态）。

## 负面发现与后续清单

- 第二梯队 64 处（玩家交互/低频）归档：合成簇（CraftItem +
  PrepareItemCraft 同点双事件，需分别判门）、net/java 包
  处理器 21 处、advancement、mob 行为 7 处、command 2 处、
  piston/tnt/note/dispenser/spawn 特殊面——事件 API 语义与
  热度均低，后续按需收编；
- 99 个 clone 上下文点未逐点甄别（`ItemStack` 带组件克隆
  vs `Arc` 计数克隆）——克隆面另立轮次评估；
- `Arc<Player>`/`Arc<World>` 克隆在事件构造中是引用计数而非
  堆分配，不属本通病清理范围。

## 提交

- `perf(world)`：生成/掉落/战利品事件订阅者先行（5 处）；
- `perf(entity)`：拾取/效果施加/姿态事件订阅者先行（3 处）；
- `perf(spawner)`：自然刷怪与刷怪笼事件订阅者先行（5 处）；
- `perf(container)`：漏斗吞吐与熔炉系事件订阅者先行（5 处）+
  信标效果（2 处）；
- `perf(benchmark)`：`auto_path_event_gate` 双臂基准（第 34 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.45 dist。
