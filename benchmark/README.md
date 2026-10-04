# Papokin 性能基准程序

本目录存放性能优化的量化基准（对比报告输出至 `note/report/perf/`）。

## region_cache —— region 序列化器缓存内存基准（轮次 1）

量化「区块序列化器缓存字节预算」优化（`LevelConfig.cache_max_mb`）的内存收益：

- **工作负载**：128 个 region × 64 区块 × 8 KiB 不可压缩负载（缓存总账约 64 MiB），
  全部区块 watched（模拟玩家加载）；3 轮「增量保存（产生 pending）+ 强制落盘（自动保存）」；
  收尾抽样读回**逐字节校验**（数据完整性闸门）。
- **对比口径**：同一二进制以 `--budget-mib 0 / 32 / 8` 各跑一遍（独立进程，RSS 互不染指）。
  `0` = 无界缓存（优化前 0.3.14 行为），`32/8` = 优化后不同预算。
- **指标**：峰值/结束 RSS（sysinfo 实测进程内存）、管理器缓存总账（`cached_bytes_total`）、
  驱逐计数、各阶段耗时。

```sh
cargo run --release -p papokin-benchmark --bin region_cache -- \
    --out note/report/perf/round1-region-cache.json
```

单点复现（子进程模式）：

```sh
./target/release/region_cache --child --budget-mib 32 \
    --regions 128 --chunks 64 --payload-kib 8 --rounds 3
```

输出最后一行为单次结果 JSON；父进程模式汇总三档预算并打印 Markdown 对比表。
