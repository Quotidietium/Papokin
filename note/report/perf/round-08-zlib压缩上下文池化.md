# 轮次 8：zlib 压缩上下文线程池化

日期：2026-10-04　类型：内存优化（CPU 顺带为正）　状态：已落库并过门禁

> **2026-10-04 勘误（轮次 9 修订）**：本报告的 -99.2% 为单线程基准模型
> 结果。生产压缩实际走 `frame_batch_maybe_offload` 的 `spawn_blocking`
> （tokio 阻塞池按需扩张，tick 齐发时线程数≈连接数），线程局部池在
> 满负载下收益归零（轮次 9 基准 W=128 场景实测 65.1 MiB ≈ 旧行为）。
> 池化方向与逐包无状态语义不变，池拓扑已在轮次 9 修正为**全局有界池**
> （16 份封顶，驻留与线程/连接数无关），压缩暂存同轮捆绑入池。见
> [round-09-压缩资源全局有界池](round-09-压缩资源全局有界池.md)。
> 下文保留原实现与基准的原始记录，阅读时请以轮次 9 为现行实现。

## 背景与侦察

协议开启压缩后（登录阶段 `set_compression` 即启用，Papokin 默认阈值 256 B、级别 6），
每条连接的 `TCPNetworkEncoder` 以字段形式常驻一份 flate2 `Compress` 压缩上下文
（miniz_oxide 的 LZ 字典窗口 + 哈希/链表 + 待决缓冲）。实测单个上下文 **320,744 B
（约 313 KiB）**——比侦察估算（100–256 KiB）还大——且自压缩启用起随连接常驻，
与连接是否真在收发可压缩流量无关。128 条已压缩连接即白占约 39 MiB；
解码侧（`async_compression` 的流式 `ZlibDecoder`）跨包有状态、不可池化，不在本轮范围。

**安全性依据（逐行核对线上语义）**：编码侧对压缩上下文的用法逐包无状态——
`compress_packet_data` 每包先 `compressor.reset()`（清空字典与历史，等价于全新流），
再以 `FlushCompress::Finish` 单次收尾并要求 `Status::StreamEnd`。因此任意两份同级别的
上下文处理同一输入产出**逐字节相同**的输出，上下文在连接间可互换。
（旧实现跨包复用同一上下文本就依赖 `reset()` 的该语义；池化只改变「用哪一份
上下文对象」，不改变每包的流语义。）

## 实现

`crates/papokin-protocol/src/java/packet_encoder.rs`：

- 删除编码器的 `compressor: Option<(CompressionLevel, Compress)>` 常驻字段；
- 新增线程局部池 `COMPRESSOR_POOL`（`RefCell<Vec<(CompressionLevel, Compress)>>`，
  每线程上限 4 份——稳态每线程在役级别仅一种，上限纯防御）；
- `PooledCompressor` 守卫：按级别检出（池中同级条目 `swap_remove` 复用，否则新建），
  析构归还（池满则释放）。检出/归还均发生在 `compress_packet_data` 单次同步调用内，
  不跨 `await`、不跨线程，无锁竞争；
- `compress_packet_data` 的 `reset()` + `Finish` + `StreamEnd` 校验逐字未动，
  scratch 缓冲治理（轮次 1 的 256 KiB 留存上限）不受影响。

选择线程局部池而非全局池：编码发生在 tokio 工作线程上，线程局部池天然免锁、
免跨线程迁移问题（守卫生命周期不跨 await）；全局池只会引入无收益的同步开销。

## 测试

- `pooled_compressor_reuse_round_trip`：归还后同级别检出命中池中条目（池长 1→0→1）；
- `pooled_compressor_respects_per_thread_cap`：6 份不同级别守卫归还后池长钳在上限 4；
- `compressed_output_identical_across_encoder_instances`：同一数据包经两个先后创建的
  编码器（后者检出前者归还的上下文）输出逐字节一致——跨实例字节全等门；
- 既有压缩/加密/大包/scratch 收缩测试（138 项 protocol lib）全部原样通过；
  world 270 + 主仓 407 测试与工作区 clippy 全绿。

## 基准（`benchmark/src/bin/compressor_pool.rs`）

真实 flate2 压缩 + 计数分配器精确测存活字节。负载：128 连接 × 25 轮 = 3200 包
交错压缩（60% 小包 0.2–2 KiB / 30% 中包 2–16 KiB / 10% 大包 16–128 KiB；
半数可压缩内容半数随机字节），级别 6，共 32,474,571 B 原始数据。
旧模式复刻「每连接常驻一份上下文」，新模式复刻生产 `PooledCompressor` 语义。

| 指标 | 旧（每连接常驻） | 新（线程池化） | 变化 |
| --- | ---: | ---: | ---: |
| 压缩上下文存活字节 | 41,055,358 B（39.2 MiB） | 345,022 B（337 KiB） | **-99.2%** |
| 单个上下文 | 320,744 B × 128 份 | 同 × 1 份在池 | 每连接省 ≈313 KiB |
| 线上字节总数 | 16,805,125 B | 16,805,125 B | 完全相等（硬闸门） |
| 全量压缩耗时 | 370.6 ms | 335.4 ms | -9.5%（热上下文缓存友好） |

三道硬闸门全绿：3200 包逐包输出哈希全等、线上字节总数相等、前 64 包逐字节
全比对一致。吞吐不降反升：128 份上下文轮换时缓存/分支预测全冷，池化后
单一热上下文驻留，属顺带收益。

折算生产直觉：每连接省约 313 KiB 常驻——100 并发已压缩连接省约 31 MiB；
代价是每线程最多 4 份上下文驻留（实测单线程 1 份即够）。空闲连接不再
白占压缩状态。

数据：[round8-compressor-pool.json](round8-compressor-pool.json)

## 提交

- `perf(protocol)`：压缩上下文线程池化（实现 + 三项单测同 commit）
- `perf(benchmark)`：compressor_pool 基准（双闸门 + 计数分配器实测）
- `docs(note)`：本报告、note/README 轮次表、note/05 数据流批注
