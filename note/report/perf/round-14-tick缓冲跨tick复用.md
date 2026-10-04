# 轮次 14：世界 tick 玩家快照与方块实体活跃集跨 tick 复用

> 日期：2026-10-05 ｜ 主题：CPU + 内存（世界 tick 热路径逐 tick 分配流失）
> 基线：轮次 13 收官（0.3.24）；本轮改动落库后以 0.3.25 发布
> 基准程序：[benchmark/src/bin/tick_buffer_reuse.rs](../../../benchmark/src/bin/tick_buffer_reuse.rs) ｜ 原始数据：[round14-tick-buffer-reuse.json](round14-tick-buffer-reuse.json)

## 1. 优化点与动机（生产可达性已验证）

沿用轮次 13 确立的「先验证生产可达，再落库」纪律。`World::tick`
（`world/mod.rs`）每 tick 无条件执行两处**逐 tick 新建 `Vec`**：

1. **玩家快照**（`players_cache`）：`players.par_iter().map(..).collect()` 收集
   `(player, pos, bb, chunk_pos)` 四元组，供实体 tick 的碰撞检测
   （O(实体数×玩家数) 嵌套比较）使用。规模 = 玩家数。
2. **方块实体活跃集**（`block_entities`）：遍历活跃区块收集
   `Arc<dyn BlockEntity>`，供统一 tick 与 `flush_comparator_updates` 排空。
   规模 = 活跃方块实体数（大型自动化装置可达数千）。

两者均为**逐 tick 重建、tick 结束即弃**的临时 `Vec`——容量从 0 反复
grow，是 tick 热路径的常态分配流失。生产可达性确认：两处在 tick 主循环
无条件执行，规模随在线玩家与自动化规模线性放大。

## 2. 改动内容（papokin/src/world/mod.rs）

新增 `TickScratch` 结构（`players_cache: Vec<PlayerSnapshot>` +
`block_entities: Vec<Arc<dyn BlockEntity>>`），作为 `World` 的
`tick_scratch: Mutex<TickScratch>` 字段**跨 tick 驻留**；两处收集点改为
`clear()` 后 `extend()` 重填——容量驻留、消去反复 grow 的分配。

**关键设计点：**

- **单守卫两处共用**：tick 单线程顺序执行，两处收集（玩家快照、方块实体）
  在同一 `Mutex` 守卫的生命周期内先后完成，全程仅一次锁获取/归还。
- **快照语义等价**：`players_cache` 元素含 `Arc<Player>`——旧实现
  `par_iter().map(|player| (player, ..))` 对 `&Arc<Player>` 解引用拷贝
  `Arc`（等价 clone），新实现 `iter().map(|player| (player.clone(), ..))`
  显式 clone，语义一致；`clear()` 丢弃旧快照、`extend` 重建，与逐 tick
  新建的中间态（tick 内快照固定）完全等价。
- **串行化取舍**：`extend` 只能接串行 `Iterator`，故玩家快照收集由
  `par_iter` 改 `iter`。玩家数（数十）下串行收集本就零显著成本，
  并行收益可忽略；方块实体收集原本就是串行遍历。
- **`flush_comparator_updates` 快照依赖**：该函数要求「本 tick 的方块实体
  快照」以避免 tick 中新增实体被提前排空——`clear()`+`extend()` 重建的
  恰是同 tick 快照，语义不变。

## 3. 兼容性论证（红线 2）

- **行为逐元素全等**：基准对两形态的「碰撞判定结果指纹（O(E×P) 全程）
  + 活跃集内容指纹」做 FNV-1a 逐元素比对，**全等**。
- **无功能丢失**：玩家快照与方块实体活跃集的元素、顺序、消费方式
  （碰撞检测 `break`、`flush_comparator_updates` 遍历）均未变。
- **锁安全**：`tick_scratch` 仅在 tick 主循环两处访问，单线程顺序，
  无嵌套锁、无跨 tick 持有（守卫随 tick 段结束归还）；`TickScratch`
  字段私有，不暴露给插件/API。
- **无内存驻留膨胀**：`clear()` 仅清长度、驻留容量——容量上界即历史
  峰值玩家数/活跃实体数，与旧实现每 tick grow 到的容量相同，只是把
  「分配-丢弃-再分配」改为「驻留复用」。

## 4. 基准结果

负载：600 tick ×（32 玩家 + 800 实体 + 1500 方块实体）。

| 形态 | 分配次数 | 分配字节 | 每 tick 分配 | 每 tick 字节 |
|---|---|---|---|---|
| 逐 tick 新建（现状） | 1200 次 | 7,814,400 B | 2.0 次 | 13,024 B |
| 跨 tick 复用（本轮） | **0 次** | **0 B** | 0.00 次 | **0 B** |

- **稳态零分配**：复用缓冲在首次 grow 后（热身 tick 完成）整个 600 tick
  测量窗内零分配、零字节——容量驻留后 `clear()`+`extend()` 不再触发任何
  堆分配。
- **每 tick 消去 13 KiB 临时分配**（2 次 Vec grow）；以 20 Hz × 世界数计，
  每世界每秒消去约 260 KiB 临时分配，自动化规模越大收益越大。
- **内容门全等**：碰撞判定结果（O(E×P) 全程）与活跃集内容逐元素一致。

## 5. 测试与门禁

- 行为等价由基准双指纹门（碰撞判定 + 活跃集逐元素全等）兜底。
- 门禁：`papokin` 407 测试全绿；`cargo clippy -p papokin --all-targets`
  零告警（`TickScratch` 字段私有、`PlayerSnapshot` 类型别名消解
  type_complexity）。

## 6. 结论

本轮延续轮次 13 的正结果，把「跨 tick 复用」形态从计划刻集合推广到
tick 热路径的两个主力临时 `Vec`（玩家快照 + 方块实体活跃集）。两处
逐 tick 分配流失（每 tick 13 KiB）收编为驻留复用（稳态零分配），快照
语义、碰撞判定、比较器排空行为逐元素全等。改动集中在 `world/mod.rs`
一处 `TickScratch` 字段与两个收集点，侵入面小、收益生产可达。
