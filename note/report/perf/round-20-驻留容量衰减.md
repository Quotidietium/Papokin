# 轮次 20：驻留缓冲容量衰减治理

> 日期：2026-10-04 ｜ 主题：内存（驻留容量） ｜ 闸门：见末节
> 基准：[benchmark/src/bin/resident_capacity_decay.rs](../../../benchmark/src/bin/resident_capacity_decay.rs)
> 数据：[round20-resident-capacity-decay.json](round20-resident-capacity-decay.json)

## 问题

轮次 17–19 把每 tick 的临时分配全部改为「驻留缓冲」模式（`Mutex`/`thread_local`
持有的 `Vec`/`HashMap` 字段，每 tick `clear()` 重填）。分配速率由此归零，
但驻留容器**永久保留历史峰值容量**：一次爆炸（上万方块变更）、一次传送
（单玩家单批 500 区块编码）、一次玩家涌入，都会把对应缓冲的容量顶到峰值，
而负载回落后这些容量永不释放——分配速率换成了驻留内存占用。

侦察确认的只增不缩面（本轮收编前）：

| 驻留结构 | 峰值来源 | 峰值量级 |
|---|---|---|
| `BlockUpdateFlushScratch.changes` 与 `unsent_block_changes`（swap 对） | 爆炸/大规模方块事件 | 万级条目，两图轮换双份 |
| `BlockUpdateFlushScratch.sections` 内层 `Vec`（逐节） | 单节变更尖峰 | 每节永久保留该节历史峰值 |
| `block_event_flush_buffer` 与 `synced_block_event_queue`（swap 对） | 活塞/音符盒事件风暴 | 两 `Vec` 双份 |
| 玩家侧 `ChunkBatchScratch` 四 `Vec` | 传送/加入突发批 500 | ≈44 KB × 在线玩家数 |
| `TickScratch` 五 `Vec` | 玩家/实体/方块实体数峰值 | 随历史最大规模 |
| `entity_tracker` `update_all_scratch` 二 `Vec` | 追踪实体/移动玩家峰值 | 同上 |
| `TickData` 三 `Vec`（`Level::get_tick_data` 重填） | 活跃区块×3 随机刻采样 | 随视距/活跃区块峰值 |

## 机制

新增 `papokin-util::capacity`（[crates/papokin-util/src/capacity.rs](../../../crates/papokin-util/src/capacity.rs)）：

- `decay_clear_vec` / `decay_clear_map`：替代驻留缓冲重填前的 `clear()`。
  清理时记录上一轮长度 `last_len`；仅当容量超过
  `max(4 × last_len, 64)` 才收缩到 `last_len + 64`。
- **4 倍滞回**是根本性安全界：摊倍增长的容器容量恒小于 4× 长度，
  因此稳态负载下**永不触发收缩**（不存在收缩-重分配抖动）；
  只有负载发生时代级回落（尖峰→稳态）才收缩，且落点保留
  「上轮长度 + 地板」余量，下一轮同负载重填零重分配。
- 与既有治理手段同族：`level.rs::clean_memory` 的 4096 空位收缩、
  `palette.rs` 的半满收缩、`packet_encoder.rs` 的 2× 池规则；
  本模块把「清理点衰减」统一成一个可复用原语。

应用点（16 处，全部是把既有 `clear()` 换成衰减清理，无逻辑改动）：

- `world/mod.rs`：事件冲刷缓冲、`TickScratch` 五 `Vec`、
  冲刷 `changes` 图（map 版）、`sections` 逐节内层 `Vec`、
  `sections` 超 4096 压实分支补 `shrink_to_fit`；
- `world/entity_tracker.rs`：`update_all_scratch` 二 `Vec`；
- `net/chunk_sender.rs`：`ChunkBatchScratch` 四 `Vec` 的清理点
  （候选重填/编码结果/编码输出/派发清单）；
- `papokin-world/src/level.rs`：`get_tick_data` 三 `Vec`。

刻意不收编（留存后续轮次）：`SharedChunkEncodeCache` 的 `DashMap`
桶容量（并发收缩须锁全分片，需单独论证）、`seen_by` 等 `DashSet`
（事件驱动增删，非重填模式）、`thread_local` 按盒暂存（随线程退出
回收，峰值本就有界）、调用方所有的 `into` 出参（容量非我方财产）。

## 负载模型

600 tick；前 10 tick 尖峰时代（10 000 变更 / 200 节×50 / 400 事件 /
批 500×50 玩家），其后稳态时代（400 变更 / 60 节×7 / 80 事件 /
批 24×50 玩家）。两臂：无衰减（现状 `clear()`）vs 衰减（`decay_clear_*`）。
观测：尖峰末与收尾的保留容量（字节）、稳态时代分配次数（收缩事件上界）、
处理指纹（tick 内交换律折叠 + tick 间顺序链式混合，与迭代顺序无关且
不会在构造性负载下抵消退化）。

## 结果

| 指标 | 无衰减（现状） | 衰减（收编） | 变化 |
|---|---:|---:|---:|
| 尖峰末保留容量 | 8 073 216 B | 8 073 216 B | 一致（衰减永不跌破在途负载） |
| 收尾保留容量 | 8 073 216 B | 1 896 192 B | **-76.5%（4.3× 回收）** |
| 稳态时代分配 | 0 次 | 216 次 | 一次性收缩波（每驻留缓冲至多一次） |
| 第 100 tick 后分配 | 0 次 | 0 次 | 零抖动 |
| 处理指纹 | `0x5767d6608803ed31` | `0x5767d6608803ed31` | 全等 |

收尾保留的 1.9 MB 构成：64 元素容量地板内的活跃缓冲（稳态负载本身
+ 地板余量）与冷节内层 `Vec`（地板之下不收缩，每节约 0.9 KB）——
均为策略刻意的下限保留，换取稳态零收缩。

## 闸门

| 闸门 | 结果 | 证据 |
|---|---|---|
| 处理指纹全等（衰减不改变任何投递语义） | PASS | 两臂 `0x5767d6608803ed31` 一致且非退化 |
| 尖峰末保留一致（不跌破在途负载需求） | PASS | 两臂同 8 073 216 B |
| 收尾保留容量 ≥4× 回收 | PASS | 4.3× |
| 一次性收缩波 ≤232（4 冲刷模型×3 + 50 玩家×4 + 哈希表收缩余量） | PASS | 实测 216 |
| 晚段稳态零抖动 | PASS | 第 100 tick 后两臂分配均为 0 |

## 结论与后续

容量衰减以「每驻留缓冲至多一次收缩」的代价换取负载回落后 4.3× 的
保留容量回收；4 倍滞回从机理上保证稳态负载永不触发收缩（晚段稳态
两臂分配序列逐次一致可证）。与轮次 17–19 的分配速率治理互补：
速率归零之后，驻留占用也有了自动回落机制。

后续候选：① `SharedChunkEncodeCache` 的 `DashMap` 桶容量收缩（并发
收缩须锁全分片，需单独论证停顿代价）；② `entity_tracker` 的
`seen_by` 等 `DashSet`（事件驱动增删，非重填模式，需换结构或周期
压实）；③ `thread_local` 按盒暂存的跨线程峰值审计；④ 真实服务器
负载下的保留容量采样（本基准为模型负载，地板/滞回参数可再标定）。
