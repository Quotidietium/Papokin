# 轮次 13：计划刻区块集合「逐 tick 全量 collect」改按需收集

> 日期：2026-10-04 ｜ 主题：CPU + 内存（世界 tick 热路径的常态分配流失）
> 基线：轮次 12 收官（0.3.23）；本轮改动落库后以 0.3.24 发布
> 基准程序：[benchmark/src/bin/scheduled_ticks_collect.rs](../../../benchmark/src/bin/scheduled_ticks_collect.rs) ｜ 原始数据：[round13-scheduled-ticks-collect.json](round13-scheduled-ticks-collect.json)

## 1. 优化点与动机

轮次 11/12 两个负结果确立了「先验证生产可达，再落库」的纪律。本轮目标
是通过该纪律检验的真实热路径：`Level::collect_tickable_chunks`
（`level.rs`）**每个世界 tick 无条件执行**，其中处理计划刻区块的旧实现：

```rust,ignore
// 旧：先收集全部键以规避「访问 loaded_chunks 时持 DashSet 分片锁」死锁
let scheduled_chunk_pos: Vec<_> = self.chunks_with_scheduled_ticks.iter().map(|p| *p).collect();
for pos in scheduled_chunk_pos {
    /* 对多数仅访问，仅少数 remove */
}
```

`for` 循环里**仅当区块计划刻耗尽或已卸载时才 `remove`**——多数 tick 一个
都不用移除，全量 `collect` 是纯分配流失（每 tick 一次 `Vec` 分配 + 全集合
拷贝）。计划刻区块在大面积红石/流体世界可达数千，该流失随集合规模与
tick 频率（20 Hz × 世界数）线性放大。

## 2. 改动内容（papokin-world/src/level.rs）

把「逐 tick 全量 collect」改为**按需收集**：遍历时只把本 tick 需移除的键
（计划刻耗尽 / 区块已卸载）压入一个小 `Vec`（常态为空、零分配），遍历后
统一移除。

**死锁规避语义不变**：移除仍推迟到 `loaded_chunks` 访问之外统一进行，
遍历过程不持 `DashSet` 分片锁访问 `loaded_chunks`（旧实现收集全部键也是
为此，新实现只需收集待移除键即达同样目的）。

## 3. 兼容性论证（红线 2）

- **行为逐元素全等**：对「本 tick 需移除的键集合」的判定逻辑一字未改
  （`!has_ticks() && !has_ticks()` 或已卸载），仅收集时机从「先全量后筛选」
  改为「边遍历边筛选」。基准对两形态的「需移除键集合指纹 + 访问序列指纹」
  做 FNV-1a 逐元素比对，**全等**。
- **移除时序等价**：旧实现边遍历边 `remove`（借全量快照规避分片锁），
  新实现遍历后统一 `remove`（借按需收集规避）——两者对同一 tick 内后续
  逻辑（`sort_unstable`、返回 ticks）均不可见。
- **无副作用**：`block_ticks`/`fluid_ticks` 的 `step_tick` 调用顺序、
  `chunks_with_scheduled_ticks` 的最终内容均不变。

## 4. 基准结果

负载：2048 个计划刻区块 × 600 tick，每 tick 4 个需移除（1% 稳定滴漏，
模拟耗尽/卸载常态）。

| 形态 | 分配次数 | 分配字节 | 每 tick 分配 | 每 tick 字节 |
|---|---|---|---|---|
| 全量 collect（现状） | 1200 次 | 9,849,600 B | 2.0 次 | 16,416 B |
| 按需收集（本轮） | **600 次** | **19,200 B** | 1.0 次 | **32 B** |

- **分配次数 -50%**（每 tick 消去 1 次全集合 Vec 分配）；
- **分配字节 -99.8%**（每 tick 16.4 KiB → 32 B：按需 Vec 常态为空，仅
  移除滴漏时按 4 个键 × 8 B 分配一次）；
- 生产语义折算：以 20 Hz × 2048 计划刻区块计，每世界每秒消去约
  20 次全量 collect / 320 KiB 临时分配；集合越大（大面积红石）收益越大。

## 5. 测试与门禁

- 行为等价由基准双指纹门（移除集合 + 访问序列逐元素全等）兜底；
  移除语义覆盖「耗尽」与「已卸载」两分支（旧实现同款）。
- 门禁：`papokin-world` 270 测试全绿；`cargo clippy -p papokin-world
  --all-targets` 零告警。

## 6. 结论

本轮是「生产可达性」纪律（轮次 11/12 沉淀）下的首个**正结果**：目标
在每 tick 无条件执行、收益真实、改动仅 15 行。计划刻集合的常态分配
流失（每 tick 16.4 KiB）收编为按需滴漏（每 tick 32 B），死锁规避语义
不变，行为逐元素全等。
