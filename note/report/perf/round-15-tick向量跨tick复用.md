# 轮次 15：世界 tick 剩余六处逐 tick 向量跨 tick 复用

> 日期：2026-10-05 ｜ 主题：内存 + CPU（世界 tick 热路径逐 tick 分配流失·续）
> 基线：轮次 14 收官（0.3.25）；本轮改动落库后以 0.3.26 发布
> 基准程序：[benchmark/src/bin/tick_vec_reuse.rs](../../../benchmark/src/bin/tick_vec_reuse.rs) ｜ 原始数据：[round15-tick-vec-reuse.json](round15-tick-vec-reuse.json)

## 1. 优化点与动机（生产可达性已验证）

延续轮次 13/14 的「先验证生产可达，再落库」纪律与「跨 tick 复用」形态。
轮次 14 收编玩家快照与方块实体活跃集后，`World::tick` 每 tick 仍执行
以下**逐 tick 新建 `Vec`**（前三处 `papokin/src/world/mod.rs`，第四处
`papokin-world/src/level.rs`）：

1. **可 tick 实体过滤集**（`tickable`）：对全量实体按「位于活跃且已加载
   区块」过滤后 `collect()`，规模 = 实体数 E。**无条件**执行。
2. **自然生成候选区块集**（`spawning_chunks`）：按活跃区块收集
   `(pos, Arc<ChunkData>)`，规模 = 活跃区块数 A。`spawn_list` 非空即执行
   ——默认游戏规则（`spawn_mobs=true`）下**等价于每 tick 执行**。
3. **活跃区块坐标集**（`active_chunks_vec`）：为 `inhabited_time` 自增把
   `FxHashSet` 拷贝成 Vec，规模 = A。**无条件**执行。
4. **计划刻/随机刻数据**（`Level::get_tick_data` 返回的 `TickData`）：
   每 tick 新建三个 Vec，其中 `random_ticks` 预分配 A×3。**无条件**执行。

六处均为逐 tick 重建、tick 结束即弃的临时 `Vec`——容量从 0 反复 grow，
是 tick 热路径的常态分配流失。生产可达性确认：四处均在 tick 主循环
无条件（或默认配置下等价于无条件）执行，规模随实体数与活跃区块数
线性放大。

## 2. 改动内容

### papokin-world/src/level.rs

- `TickData` 派生 `Default`（供复用缓冲零值初始化）。
- `Level::get_tick_data` 由「返回新建 `TickData`」改为**重填模式**：
  新参数 `tick_data: &mut TickData`，三 Vec `clear()` 后按原逻辑填充，
  `random_ticks` 以 `reserve(活跃区块数×3)` 补足容量（复用形态下容量
  已足时 `reserve` 为零成本空操作）。唯一调用方为 `World::tick`。

### papokin/src/world/mod.rs

`TickScratch`（轮次 14 引入的跨 tick 驻留缓冲）扩四字段：
`entities_cache`（实体过滤集）、`tick_data`（计划刻/随机刻三 Vec）、
`spawning_chunks`（生成候选）、`active_chunks_cache`（活跃坐标集）。
四处收集点改为 `clear()` 后重填。

**关键设计点：**

- **锁分两程持有**：tick 单线程顺序执行。第一程自玩家快照起持有，
  覆盖实体过滤集收编（该段本就在守卫生命周期内，零新增锁开销）；
  第二程自计划刻段起重取锁，覆盖 `TickData` 重填、生成候选、活跃
  坐标集三处，至 `inhabited_time` 自增结束归还。除 `World::tick` 外
  无任何代码获取该锁，重取无争用、无重入。
- **并行收集改 `par_extend`**：实体过滤集大集合分支原为
  `par_iter().filter_map(..).collect()`；复用形态下 Vec 无法直接接
  并行迭代器产物，改用 rayon `ParallelExtend`（`par_extend`）——
  先各线程收集再归并追加到驻留 Vec，容量驻留不丢并行性。串行分支
  （实体数 ≤ 256）仍 `extend`。
- **`TickData` 重借绑定**：计划刻段以 `let tick_data = &mut tick_scratch
  .tick_data;` 重借，后续 `split_off`（计划刻积压顺延）/`sort_unstable`/
  分批执行代码零改动。
- **`spawn_list` 空集短路保留**：`spawning_chunks` 仅在 `spawn_list`
  非空时重填，空集分支连 `clear()` 都不执行（驻留长度为零，无成本）。

## 3. 兼容性论证（红线 2）

- **行为逐元素全等**：基准对两形态的「实体过滤集、生成候选、活跃坐标、
  TickData 三 Vec」四类产出做 FNV-1a 逐元素指纹比对，**全等**。
- **无功能丢失**：六处的元素、顺序（`TickData` 内 `sort_unstable` 仍在
  重填后执行）、消费方式（实体分批 tick、生成洗牌、原子自增、计划刻
  积压顺延）均未变。
- **锁安全**：第二程守卫持有期间执行计划刻与生成行为（含 rayon 并行
  批），但全仓仅 `World::tick` 获取 `tick_scratch` 锁，无重入路径
  （tick 不被计划刻/生成行为回调），不构成死锁。
- **无内存驻留膨胀**：容量上界即历史峰值规模，与旧实现每 tick grow 到
  的容量相同，只是把「分配-丢弃-再分配」改为「驻留复用」。

## 4. 基准结果

负载：600 tick ×（800 实体 + 600 活跃区块 + 64/32 计划刻 + 1800 随机刻
采样/tick）。

| 形态 | 分配次数 | 分配字节 | 每 tick 分配 | 每 tick 字节 |
|---|---|---|---|---|
| 逐 tick 新建（现状） | 8414 次 | 49,387,776 B | 14.0 次 | 82,313 B |
| 跨 tick 复用（本轮） | **0 次** | **0 B** | 0.00 次 | **0 B** |

- **稳态零分配**：复用缓冲首次 grow 后（热身 tick 完成）整个 600 tick
  测量窗内零分配、零字节。
- **每 tick 消去约 80 KiB 临时分配**（14 次 Vec 分配/grow）；以 20 Hz
  计每世界每秒消去约 1.6 MiB 临时分配，与轮次 14 合计约 1.9 MiB/s。
- 每 tick 14 次分配中约 8 次来自六 Vec 的容量倍增 grow——复用形态
  将其全部消去。
- **内容门全等**：四类产出指纹逐元素一致。

## 5. 测试与门禁

- 行为等价由基准四指纹门（实体过滤集/生成候选/活跃坐标/TickData）
  逐元素全等兜底。
- 门禁：`papokin-world` 270 与 `papokin` 407 测试全绿；
  `cargo clippy --workspace --all-targets` 零告警。

## 6. 结论

本轮把「跨 tick 复用」形态推广到 tick 热路径剩余的全部主力临时
`Vec`（实体过滤集、生成候选、活跃坐标集、`TickData` 三 Vec 共六处），
每 tick 约 80 KiB 分配流失收编为驻留复用（稳态零分配），行为逐元素
全等。至此 `World::tick` 主循环的逐 tick 向量分配面基本肃清：剩余
分配点或为冷路径（踢出/迁移），或规模极小（计划刻移除键常态为空）。
改动集中在 `TickScratch` 四字段扩列与四个收集点，加 `get_tick_data`
一处签名改重填，侵入面小、收益生产可达。
