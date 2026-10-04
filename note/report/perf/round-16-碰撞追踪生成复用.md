# 轮次 16：实体移动碰撞收集 / 追踪器快照 / 生成列表跨 tick 复用

> 日期：2026-10-05 ｜ 主题：内存 + CPU（实体 tick 与追踪热路径分配流失）
> 基线：轮次 15 收官（0.3.26）；本轮改动落库后以 0.3.27 发布
> 基准程序：[benchmark/src/bin/collision_tracker_scratch.rs](../../../benchmark/src/bin/collision_tracker_scratch.rs) ｜ 原始数据：[round16-collision-tracker-scratch.json](round16-collision-tracker-scratch.json)

## 1. 优化点与动机（生产可达性已验证）

延续「先验证生产可达，再落库」纪律。本轮收编三处逐 tick/逐调用分配：

1. **实体移动碰撞收集**（`world/mod.rs` `get_block_collisions`）：每次调用
   返回两个新建 `Vec`（碰撞形状 + 方块位置映射）。五个调用点全部为热路径
   ——实体移动 `adjust_movement_for_collisions`（**每个移动实体每 tick**）
   与四个弹射物路径（弹射物基类/箭/三叉戟/浮漂，**每个飞行弹射物
   每 tick**）。站立/行走实体每 tick 与地面碰撞，两 Vec 均非空、必分配。
   规模 = 移动实体数 ×2 次/tick。
2. **实体追踪器快照**（`entity_tracker.rs` `update_all`）：每 tick
   `entity_map.iter().map(clone).collect()` 收集全部受追踪实体（E 规模）
   外加 `moved_players` 新建 Vec，**无条件**执行。
3. **生成列表**（`World::tick` 生成段）：每 tick 新建
   `Vec<&'static MobCategory>` 并 `Arc::new` 包装、批次闭包逐批 Arc
   clone——而下游 `tick_spawning_chunk` 形参本就是 `&Vec` 借用，Arc
   纯属多余。**无条件**执行。

## 2. 改动内容

### 碰撞收集：线程局部暂存 + 重填模式

- `get_block_collisions` 改 `get_block_collisions_into(bounding_box,
  entity, &mut collisions, &mut positions)`：清空重填调用方缓冲。
- 新增模块级线程局部暂存 `BLOCK_COLLISION_SCRATCH: RefCell<(Vec<
  BoundingBox>, Vec<(usize, BlockPos)>)>`（const 初始化）与门面方法
  `World::with_block_collisions(bb, entity, f)`：暂存内收集、闭包内
  就地消费（切片借用）。
- 五个调用点全部迁移：实体移动（`entity/mod.rs`）Y 轴位置映射游标由
  `into_iter()` 改 `iter().copied()`；四个弹射物点消费逻辑原样搬入闭包。

**关键设计点：**

- **为何线程局部而非 `TickScratch`**：碰撞收集发生在实体/弹射物 tick
  的 **rayon 并行批**内（多工作线程并发），`World` 级单缓冲无法满足；
  线程局部使每 rayon 工作线程持一份暂存，容量随线程驻留，天然无锁。
- **无重入**：收集（`get_block_collisions_into` 内含细雪形状按实体类型
  裁决，纯读）与消费（碰撞时间计算/射线求交，纯数学）均不回调碰撞
  收集，`RefCell` 借用不跨出单次收集，无二次借用 panic 路径。
- **语义逐元素全等**：收集逻辑（方块迭代序、形状过滤、位置映射
  `(累计形状数, BlockPos)`）一行未动，仅容器来源变更。

### 追踪器快照：驻留缓冲

- `EntityTracker` 新增私有 `update_all_scratch: Mutex<(Vec<Arc<
  TrackedEntity>>, Vec<Arc<Player>>)>`，`update_all` 改 clear 重填。

**关键设计点：**

- **持锁迭代安全论证**：锁仅 `update_all` 获取；`update_players` 触发的
  阻塞插件事件回调路径（传送→注视更新→`update_player_position`）不
  触碰本缓冲，重入 `update_all` 的路径不存在（tick 不被插件同步调用）。
- **`update_player_position` 不在本轮**：该路径可被多名玩家的任务链
  并发调用且同样触发插件事件，驻留缓冲会重引入它当年靠快照规避的
  锁死类风险（见 `entity_tracker.rs` 行 657 注释），保持逐调用新建。

### 生成列表：驻留重填 + 去 Arc

- `get_filtered_spawning_categories` 改重填模式（`out: &mut Vec<&'static
  MobCategory>`，唯一调用方即 `World::tick`）。
- 列表入 `TickScratch.spawning_categories`；`spawn_batch` 闭包直传
  `&tick_scratch.spawning_categories`，删去逐 tick `Arc::new` 与逐批
  `Arc::clone`（原子计数开销同步消去）。

## 3. 兼容性论证（红线 2）

- **行为逐元素全等**：基准对两形态的「碰撞形状+位置映射、追踪器快照、
  生成列表」三类产出做 FNV-1a 逐元素指纹比对，**全等**。
- **无功能丢失**：碰撞收集内容/顺序、Y 轴支撑块裁决、水平碰撞标记、
  弹射物命中映射、追踪器可见性更新次序、生成类别过滤条件均未变。
- **无内存驻留膨胀**：暂存/驻留容量上界即历史峰值规模，与旧实现每次
  grow 到的容量相同，只是把「分配-丢弃-再分配」改为「驻留复用」。
- **线程安全**：线程局部暂存不跨线程共享；追踪器缓冲经 `Mutex` 独占
  且重入路径不存在；生成列表借用生命周期不跨出持有 `tick_scratch`
  锁的生成段。

## 4. 基准结果

负载：600 tick ×（800 移动实体 + 800 受追踪实体 + 8 生成类别）。

| 形态 | 分配次数 | 分配字节 | 每 tick 分配 | 每 tick 字节 |
|---|---|---|---|---|
| 逐 tick 新建（现状） | 1,283,735 次 | 208,998,696 B | 2,139.6 次 | 348,331 B |
| 跨 tick 复用（本轮） | **0 次** | **0 B** | 0.00 次 | **0 B** |

- **稳态零分配**：复用形态热身 tick 后整个 600 tick 测量窗零分配。
- **每 tick 消去约 340 KiB / 2140 次分配**——为迄今单轮最大每 tick
  分配账（轮次 15 为 80 KiB/14 次）；其中约 1600 次来自移动实体碰撞
  两 Vec 的新建与倍增 grow。以 20 Hz 计每世界每秒消去约 6.8 MiB 临时
  分配，移动实体越多收益越大。
- **内容门全等**：三类产出指纹逐元素一致。

## 5. 测试与门禁

- 行为等价由基准三指纹门（碰撞/快照/生成列表逐元素全等）兜底。
- 门禁：`papokin` 407 测试全绿；`cargo clippy --workspace
  --all-targets` 零告警。

## 6. 结论

本轮把复用形态推广到三类新场景：**rayon 并行上下文**（线程局部暂存
收编实体移动碰撞收集，每 tick 约 1600 次分配的主要来源）、**追踪器
快照**（驻留重填）、**假共享包装**（生成列表去 Arc 直传借用）。每
tick 约 340 KiB / 2140 次分配流失收编为稳态零分配，行为逐元素全等。
配合轮次 14/15，`World::tick` 主循环与实体 tick 的常态分配面基本
肃清；剩余已知分配点（`update_player_position` 快照、数据包构造、
插件事件对象）或为并发/重入安全所必需，或具逐变更语义，不属于
流失。改动集中在五个碰撞调用点、追踪器一处、生成段一处，侵入面小、
收益生产可达。
