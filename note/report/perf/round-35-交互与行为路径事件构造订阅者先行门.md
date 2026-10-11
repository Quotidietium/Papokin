# 轮次 35 · 交互与行为路径事件构造的订阅者先行门（通病系统性清扫·第二+第三梯队收官）

## 背景：通病残余人口的彻底清零

轮次 34 收编第一梯队 19 处后归档了第二梯队 64 处。本轮按在册
清单启动第二梯队收编，收编完成后做**收尾全仓重扫**，发现文件级
残余远超归档口径（约 140 个 fire 点所在文件无任何 `has_handlers`）。
以「事件构造携带分配」为精确口径重扫（构造回看 22 行含
`.to_string()`/`format!`/`.clone()`/`String`/`vec!` 等分配指示；
纯 Copy 字段构造零分配、`fire_blocking` 内部短路已覆盖，无收益
不计）得到**第三梯队 76 处**。按战役条款「每一轮的优化方面必须
做完」，本轮将第二+第三梯队一并收编，通病至此**全仓清零**。

### 收编总量与分布

| 梯队 | 处数 | 覆盖面 |
|---|---|---|
| 第二梯队（在册 64） | 69（含 setworldspawn/whitelist 附带收编 5） | 玩家容器/合成/锻造/牌匾/自定义载荷/权限、net/java 包处理器 21、entity 行为 15、unleash/桶/音符盒/TNT/活塞/发射器、gamerule/locate/server/context |
| 第三梯队（扫描收编） | 74（76 − 2 处 Copy 误报） | block 层 32、entity 层 20、item 层 11、net 层 9、world 层 4 |
| **合计** | **约 143 处 / 60+ 文件** | |

Copy 误报勘验 2 处：`map.rs:51`（`MapInitializeEvent` 仅 i32 字段，
`const fn new` 零分配）、`mob/mod.rs:1265`（`EntityBreedEvent` 三
i32 字段）。同名 mob 辅助 fire 函数中的 `enter_love_mode`/
`break_door`/`enter_block`/`interact` 同为 Copy 构造，按口径豁免。

## 改动范式（延续轮 34，四种形态）

统一门：`if let Some(server) = X.server.upgrade() && server
.plugin_manager.has_handlers::<E>()` 先行（已在 `server` 作用域
内的直接 `server.plugin_manager.has_handlers::<E>()`）。四种收编
形态：

1. **A 型（构造本在 if 内）**：链上追加 `&& has_handlers`——
   桶装/桶倒（门置于 `get_player_by_uuid` 之前，无监听连玩家
   查找也省）、告示牌进入/农田两处/树叶/火焰/作物/甜浆果/
   酿造×2/营火×2/草床/发射器失败/雪傀儡/鸡/鸡蛋/剪刀/三叉戟/
   编辑书/交互坐标/挖掘开始与中止/跟踪器 untrack 等；
2. **B 型（构造在门外，纯 cancelled 回读）**：let-cancelled
   表达式（`let cancelled = if 门 { 构造; fire; event.cancelled }
   else { false };`）或门内嵌 `return`/`continue`——苦力怕/龙/
   炼药锅/堆肥桶/花盆×2/讲台/TNT×2/活塞×2/钟鸣响/幽匿×2/
   试炼笼/流体/效果云/女巫×3/掉落物合并/展示框×3/悬挂物放置×2/
   弩/选交易/挥手/跟踪器 show/hide/旁观等；
3. **C 型（纯通知无回读）**：门包构造——白名单×4/
   setworldspawn/server×3（资源重载/连接关闭/难度变更）/
   context 命令注册/gamerule/龙（嵌入 return 形）；
4. **D 型（特殊回读管道）**：`Option`/元组管道回填或按构造值
   续行——locate `results.first()`（无监听直接沿用定位目标）、
   告示牌命令 `command` 重写回读、堆肥桶 `will_raise_level`、
   块掉落 `items`、块经验 `exp`、宝库掉落 `items`、猪灵
   `outcome`、经验球 `new_cooldown`/`amount`（amount 的
   `.max(0)` 钳制在无监听臂同样保持）、滑翔 `is_gliding`、
   创造槽/命令补全（前轮）等。

### 逐点语义核对（红线：行为/UX 零变化）

- 门谓词与 `fire_blocking` 内部短路完全同源（同为
  `handlers.has_handlers(E::get_name_static())`）——无监听时
  旧路径构造的事件必被丢弃、`cancelled=false`、可变字段保持
  构造值，新旧路径续行值逐位一致；
- 悬挂物双事件（HangingBreak + HangingBreakByEntity）**各自
  独立判门**：监听可能只订其中一个；首事件分支以 `entity_arc
  .clone()` 参与构造（`entity_arc` 本体非 Option，编译期纠正
  过一次 as_ref 误形）；
- 女巫饮药臂的 `effects` 解析（`read_potion_effects`）保持在
  门外——它是药水应用所需而非事件构造，仅事件构造与 fire
  入门；
- `player_action` ReleaseItemInUse 分支顺带把只为取 server 的
  `world.load_full()`（Arc 计数克隆）降为 `world.load()`（Guard
  读借用），语义不变少一次引用计数往返；
- 桶装/桶倒的门置于 `get_player_by_uuid` 之前：无监听时 UUID
  索引查找一并省去（查找无副作用，取消语义依赖事件回读而
  非查找结果，等价性成立）；
- 白名单状态更新两处把门提升到 `for` 循环外（每命令一次而非
  每成员一次），循环体语义不变。

## 双臂基准

`benchmark/src/bin/interaction_event_gate.rs`（第 35 个 bin）。
「玩家交互 + 世界行为」混合轮模型（无订阅者）：每轮复刻真实
构造形态——告示牌命令预处理（command clone）、桶装/桶倒
（registry_key to_string）、发射器（同）；每 4 轮活塞伸缩
（format!×2）；每 8 轮龙相位（format!×2）+ 苦力怕充能
（"LIGHTNING"）；每 64 轮鸡下蛋。保底 4 String/轮。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 两臂语义指纹一致 | PASS（0x9d8e109ff1080d82） |
| G2 | 旧臂 ≥ 轮数×4、新臂 == 0 | PASS（514 063 ≥ 400 000；0） |
| G3 | 新臂耗时 ≤ 旧臂 | PASS（0.31 ms ≤ 15.03 ms） |
| G4 | 旧臂毛字节 > 新臂 | PASS（8.18 MB > 0） |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 混合轮 ×100 000 | 514 063 allocs / 8.18 MB / 15.03 ms | **0** / **0** B / 0.31 ms | 分配 −100%、耗时 **48.4×** |

旧臂 5.14 String/轮 = 4 保底 + format! 超容量增长分配与
稀疏周期（活塞/龙/下蛋）摊入。

## 负面发现与后续清单

- **fire 系通病全仓清零**：重扫仅余 `map.rs:51`（Copy 字段
  误报已勘验）。后续新增 fire 点须延续门范式（代码评审项）；
- 99 个 clone 上下文点未逐点甄别（`ItemStack` 带组件克隆
  vs `Arc` 计数克隆）——克隆面另立轮次评估，为下一轮在册
  候选；
- `Arc<Player>`/`Arc<World>` 克隆在事件构造中是引用计数而非
  堆分配，不属本通病清理范围。

## 提交

按主题细分（细粒度规范）：

- `perf(player)`：容器/合成/锻造/牌匾/权限事件订阅者先行（13 处）；
- `perf(net)`：net/java 包处理器与 play 处理器事件门（20 处）；
- `perf(entity)`：实体行为/AI/装饰物/投射物事件门（36 处）；
- `perf(block)`：方块交互/红石/流体/方块实体事件门（32 处）；
- `perf(item+command+server)`：物品使用/命令/服务器通知事件门（15 处）；
- `perf(world)`：实体跟踪器 show/hide/track/untrack 事件门（4 处）；
- `perf(benchmark)`：`interaction_event_gate` 双臂基准（第 35 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.46 dist。
