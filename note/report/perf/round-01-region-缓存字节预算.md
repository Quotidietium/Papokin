# 性能优化轮次 1：region 序列化器缓存字节预算（内存）

> 日期：2026-10-04 · 基线版本：0.3.14+1.21.11 · 优化类别：内存占用
> 基准程序：`benchmark/src/bin/region_cache.rs` · 原始数据：[round1-region-cache.json](round1-region-cache.json)

## 1. 优化点与动机

`ChunkFileManager`（`crates/papokin-world/src/chunk/io/file_manager.rs`）把 region 文件
全部区块的**压缩字节**驻留内存做写合并。既有逐出逻辑（`maybe_evict`）只在区块不再被
watch 时触发；而运行中服务器几乎全员 watched——只要区块加载着，对应 region 的
序列化器就无限驻留（`MAX_CACHED_SERIALIZERS = 1024` 只限条目数不限字节）。
note/07、09 均将此记为遗留风险（"region 文件整文件驻留内存无 LRU 上限"）。

量级估算：成型存档单 region 文件 1–8 MiB；玩家散开时 watched region 数十至数百个，
缓存可达数百 MiB 至上 GiB，且随探索范围单调增长。

## 2. 方案（红线约束下的设计）

新增**缓存字节预算 + LRU 驱逐**，核心不变式：

1. **零数据风险**：只驱逐 `can_remove`（无存活引用且 `has_pending_writes() == false`，
   即磁盘已是最新状态）的条目。驱逐不丢任何数据、**不需要任何额外写盘**；
   被驱逐条目下次访问从磁盘重读，状态完全一致。带未落盘更新的条目绝不可驱逐
   （另有既有 `flush_pending_writes` 关停兜底不变）。
2. **行为一致**：watched 语义（保存先合并进内存）不变；自动保存/卸载/save-all 节奏
   不变；区块数据、文件格式、网络协议零改动。唯一可感知差异是内存有了上界，
   以及超预算时干净 region 的读回多一次磁盘重读（毫秒级）。
3. **兼容**：`[world] cache_max_mb` 配置项（`papokin.toml`），默认 **256 MiB**/管理器
   （region 与 entities 各自记账）；`0` = 无界，恢复 0.3.14 及之前行为。
   旧配置文件缺失该字段时由 serde 默认值回填，无需迁移。
4. **低抖动**：滞回策略——超限时驱逐到预算的 80%；LRU 序号驱逐最近最少使用条目；
   锁忙条目本轮跳过（保守按 0 字节计），下轮再收。

执法点 = 缓存增长的两条路径：`get_serializer` 插入（读路径）与
`save_chunks_with` 收尾（写路径，统一一次避免并发锁车队）。
实现面：`ChunkSerializer::cached_bytes()`（anvil/linear/pump 三格式实账）、
`ChunkFileManager::{enforce_byte_budget, evicted_total, cached_bytes_total}`、
`PathFromLevelFolder` 转 pub（供基准复现真实目录布局）。

## 3. 量化结果（同工作负载对比，release 构建）

工作负载：128 region × 64 区块 × 8 KiB 不可压缩负载（缓存总账约 64 MiB），
全员 watched；3 轮增量+强制保存；收尾逐字节读回校验。

| 预算 (MiB) | 峰值 RSS | 结束 RSS | 缓存总账(收尾) | 驱逐数 | 填充耗时 | 轮次耗时 | 读回耗时 | 数据校验 |
|---:|---:|---:|---:|---:|---:|---:|---:|:--:|
| 0（=优化前） | 149.2 MiB | 150.2 MiB | 64.5 MiB | 0 | 4817 ms | 4014 ms | 9 ms | ✅ |
| 32 | 79.6 MiB | 58.1 MiB | 26.2 MiB | 462 | 4704 ms | 4424 ms | 11 ms | ✅ |
| 8 | 28.3 MiB | 25.0 MiB | 7.1 MiB | 500 | 4664 ms | 4318 ms | 9 ms | ✅ |

- **峰值 RSS：149.2 → 79.6 / 28.3 MiB（-47% / -81%）**；缓存总账严格贴合预算滞回线
  （32×0.8=25.6 ≈ 26.2；8×0.8=6.4 ≈ 7.1），机制行为与理论一致。
- 时间代价：填充/读回无回归；保存轮次 +7~10%（超预算场景下干净 region 驱逐后的
  重读开销）——默认 256 MiB 预算下正常服务器不会触发驱逐，代价只出现在真正超限的
  极端场景，与内存收益相称。
- 三档预算读回校验全部逐字节通过：驱逐零数据损失得到实证。

复现：`cargo run --release -p papokin-benchmark --bin region_cache -- --out note/report/perf/round1-region-cache.json`

## 4. 正确性论证与测试

- 新增单元测试 6 个（`file_manager.rs`）：LRU 驱逐序、pending 条目不可驱逐、
  预算 0 = 无界（旧行为）、真实 Anvil 格式端到端往返（驱逐后磁盘读回逐字节一致、
  驱逐后再写入正确重读合并）、watched+pending 条目保存中不可驱逐。
- 回归：`papokin-world` 258 测试、`papokin-config` 22 测试全绿；
  `clippy --all-targets -D warnings` 与 `cargo fmt --check` 通过。
- 安全性保持：写合并/原子写/损坏留档等既有机制零改动；`has_pending_writes` 语义
  是驱逐的硬门槛（审计 R5 确立的线性化点）。

## 5. 适用边界

- 默认 256 MiB/管理器：单维度 region+entities 两管理器，三维度共 6 份账本，
  理论上限约 1.5 GiB（旧行为无上限）；正常规模服务器（数十玩家）远低于阈值，
  驱逐不发生、零开销。
- watched 且**脏**（两次自动保存之间累积的更新）部分不可驱逐——这是写合并设计的
  固有驻留，量级由实际变更率决定，不在本轮收口范围。
- Linear/Pump 格式同样接入预算（`cached_bytes` 实账），行为一致。
