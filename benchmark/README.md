# Papokin 性能基准程序

本目录存放性能优化的量化基准（workspace 第 20 成员 `papokin-benchmark`，对比报告输出至 `note/report/perf/`）。

每个 bin 是一个**双臂对照基准**：同一负载分别跑「优化前形态」与「优化后形态」两臂，以分配计数器（次数/净驻留字节）、耗时、canonical 输出指纹（语义等价硬闸门）量化收益；闸门预登记，未达如实记 FAIL。轮次索引与全部结果见 `note/README.md` 性能优化对照表与各 `round-*.md` 报告。

## 基准清单（25 个 bin，轮次 1-25）

| bin | 轮次 | 主题 | 报告 |
|---|---|---|---|
| `region_cache` | 1 | region 序列化器缓存字节预算（`--budget-mib` 三档 RSS 对照） | round-01 |
| `connection_scratch` | 2 | 每连接收发 scratch 容量治理 | round-02 |
| `chunk_encode_cache` | 3 | 区块编码缓存容量裁剪 | round-03 |
| `chunk_cache_prune` | 4 | 区块编码缓存逐出策略 | round-04 |
| `palette_nibble` | 5 | 调色板半字节索引存储 | round-05 |
| `light_demote` | 6 | 光照容器均质归一 | round-06 |
| `shared_chunk_cache` | 7 | 共享区块编码缓存 | round-07 |
| `compressor_pool` | 8 | zlib 压缩上下文池化 | round-08 |
| `compression_resource_pool` | 9 | 压缩资源全局有界池 | round-09 |
| `frame_buffer_pool` | 10 | 组帧缓冲全局池化 | round-10 |
| `boxed_slice_alloc` | 11 | boxed-slice 双分配侦察 | round-11 |
| `heightmap_lock_batch` | 12 | 高度图锁按列聚合侦察 | round-12 |
| `scheduled_ticks_collect` | 13 | 计划刻集合按需收集 | round-13 |
| `tick_buffer_reuse` | 14 | tick 缓冲跨 tick 复用 | round-14 |
| `tick_vec_reuse` | 15 | tick 向量跨 tick 复用 | round-15 |
| `collision_tracker_scratch` | 16 | 碰撞追踪生成复用 | round-16 |
| `box_query_reuse` | 17 | 按盒查询复用 | round-17 |
| `broadcast_flush_reuse` | 18 | 广播扇出单趟化 | round-18 |
| `chunk_batch_reuse` | 19 | 区块批次缓冲与版本集 | round-19 |
| `resident_capacity_decay` | 20 | 驻留容量衰减（`decay_clear_vec/map` 原语） | round-20 |
| `encode_cache_bucket_reclaim` | 21 | 编码缓存桶回收（DashMap 滞回收缩） | round-21 |
| `world_tables_bucket_reclaim` | 22 | 世界三表桶驻留回收 | round-22 |
| `thread_local_scratch_decay` | 23 | 线程局部暂存衰减 | round-23 |
| `preserved_fields_blob` | 24 | 区块保留字段序列化驻留 | round-24 |
| `handler_sub_table` | 25 | 插件事件订阅表形态（单事件 ArcSwap） | round-25 |

## 运行方式

```sh
# 通用（多数 bin 无参数，结果自动写 note/report/perf/round<N>-*.json）
cargo run --release -p papokin-benchmark --bin <bin名>

# region_cache（轮次 1）带三档预算参数：
cargo run --release -p papokin-benchmark --bin region_cache -- \
    --out note/report/perf/round1-region-cache.json

# 单点复现（子进程模式）：
./target/release/region_cache --child --budget-mib 32 \
    --regions 128 --chunks 64 --payload-kib 8 --rounds 3
```

输出最后一行为单次结果 JSON；region_cache 父进程模式汇总三档预算并打印 Markdown 对比表。

## 编写约定

- 分配计数器（`GlobalAlloc` 转发 `System` 并计数次数/字节/释放字节，净驻留 = 毛 − 释放）；`// SAFETY:` 注释随行；
- `#![allow(clippy::print_stdout, clippy::print_stderr)]`；结果文件写入走 `match File::create` + `write_all` + `eprintln!`/`exit(1)`，不用 expect；
- 指纹用 canonical 折叠（复合标签序不敏感、列表序敏感），空集/镜像键退化坑见轮次 21/22 报告教训节；
- 闸门在 `evaluate_gates` 预登记；未达 FAIL 保留在 JSON，报告内论证收编决策。
