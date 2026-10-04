# 轮次 10：组帧缓冲全局池化

> 日期：2026-10-04 ｜ 主题：内存（每连接组帧 scratch 常驻 + 批路径逐批分配 → 统一全局有界池）
> 基线：轮次 9 收官（0.3.22）；本轮改动落库后以 0.3.23 发布
> 基准程序：[benchmark/src/bin/frame_buffer_pool.rs](../../../benchmark/src/bin/frame_buffer_pool.rs) ｜ 原始数据：[round10-frame-buffer-pool.json](round10-frame-buffer-pool.json)

## 1. 优化点与动机

轮次 9 收编了压缩侧资源（zlib 上下文 + 压缩暂存），组帧侧仍有两处遗留：

1. **`write_packet` 单包路径的常驻 `frame_scratch`**：`TCPNetworkEncoder` 每条连接
   常驻一块组帧缓冲（经轮次 2 收缩治理封顶 256 KiB）。该路径在生产中仅登录/配置
   阶段使用（唯一调用点 `pending.rs:213`），登录结束后缓冲随连接常驻空转——
   C 条连接即 C × ≤256 KiB 的稳态驻留（128 连接实测 32.0 MiB）。
2. **批路径逐批 `Vec::new()`**：`frame_packet_batch`（`net/java/mod.rs`）每批组帧
   新建一块缓冲，tick 齐发下形成稳定的分配/释放流失；单批帧最大 2 MiB，批量
   突发时分配器压力集中。

两处本质同一资源：纯字节组帧暂存，无状态、可互换。本轮新增全局有界池
`FRAME_BUFFER_POOL`（16 份封顶、池空检出永远新建不阻塞、归还时逾 2×256 KiB
收缩回 256 KiB——与轮次 9 压缩池同款治理），两条路径统一检出/归还。

## 2. 改动内容

### 2.1 `papokin-protocol`（packet_encoder.rs）

- 新增 `FRAME_BUFFER_POOL: Mutex<Vec<Vec<u8>>>`（私有静态，16 份驻留上限）
  与三个自由函数：`checkout_frame_buffer()`（池空新建）、
  `give_back_frame_buffer()`（清空+收缩治理）、`return_frame_buffer()`
  （治理+归还封顶）。
- `TCPNetworkEncoder` 移除常驻字段 `frame_scratch`；`write_packet` 改为检出
  裸缓冲→组帧→`write_frame`→显式归还（所有出口——成功/组帧失败/写帧
  失败——均归还，杜绝泄漏）。
- 删除仅用于观测常驻容量的 `frame_scratch_capacity()`（常驻字段既去，
  观测点失去意义；池语义由新增单测覆盖）。

### 2.2 `papokin`（net/java/mod.rs）

- `frame_packet_batch` 的 `Vec::new()` 改 `checkout_frame_buffer()`；
  写循环 `write_frame` 完成后 `return_frame_buffer(frame)` 归还；
  组帧/写帧失败路径（连接随即关闭）裸缓冲自然释放，不强制归还。

### 2.3 弃守卫、取裸 Vec 的设计说明

批路径 `frame_packet_batch` 的返回值 `(writer, frame, err)` 须移交帧数据
所有权（可能经 `spawn_blocking` 跨线程返回），守卫形态会侵入签名并跨
`await` 持有；裸 `Vec<u8>` + 显式归还能保持签名不动、写循环改动最小，
两条路径语义对称。归还时机因此比守卫稍晚（守卫析构 → 写循环显式调用），
对池驻留无实质影响。

### 2.4 基准适配（顺带修复）

- `connection_scratch` 基准的两处编码器留存观测点改恒 0：轮次 9 移除压缩
  暂存、轮次 10 移除组帧暂存后，per-connection 编码器侧已无可观测驻留
  （驻留全部由两个全局池以常数份承载）。
- 轮次 9 提交时漏跑 benchmark crate 的 clippy，`compression_resource_pool`
  累积的 8 个 lint 随本轮门禁一并肃清（0.3.22 发布前修复，commit 3ea785a8e）。

## 3. 兼容性论证（红线 2）

- **线上字节逐字节全等**：组帧算法（头部 VarInt 布局、拷入顺序）原样未动，
  仅缓冲来源由常驻/新建改池检出。基准内置 FNV-1a 滚动哈希 + 字节数双硬闸门，
  两策略逐包比对全等（W=16/W=128 均通过）。
- **无阻塞**：池空检出永远 `Vec::new()`，组帧路径绝不因池化等待。
- **无泄漏**：`write_packet` 所有出口均归还；池满归还直接释放，驻留封顶
  常数份。
- **错误语义不变**：帧内容错误仍由 `frame_packet` 产生，连接关闭流程不动。

## 4. 基准结果

负载：128 条模拟连接 ×（登录期 40 包 + 稳态 260 包），登录期中段含一次
1 MiB 突发大包（配方/标签同步级）；W=16（常态，4 线程池×超订）与 W=128
（极端，每连接一线程）两种拓扑。

| 拓扑 | 策略 | 组帧缓冲驻留 | 缓冲创建数 | 墙钟时间 |
|---|---|---|---|---|
| W=16 | legacy（每连接常驻） | 33,557,504 B（32.0 MiB） | 128 | 708.4 ms |
| W=16 | 全局池（本轮） | **2,883,968 B（2.75 MiB）** | 11 | 727.0 ms |
| W=128 | legacy（每连接常驻） | 33,557,504 B（32.0 MiB） | 128 | 541.9 ms |
| W=128 | 全局池（本轮） | **4,194,688 B（4.0 MiB）** | 23 | 528.6 ms |

- **驻留降幅：W=16 −91.4%，W=128 −87.5%**。legacy 恒定 128 份 × 256 KiB
  （收缩治理后的留存上限）；全局池把驻留压到 11–23 份（检出峰值并发决定，
  与连接数解耦），且 W=128 满竞争下仍只建 23 份——池复用率 99.4%。
- **墙钟时间 ±2.6% 内**：W=128 下池化反而略快（528.6 vs 541.9 ms，分配器
  压力下降抵消检出锁开销）；W=16 略慢 2.6%（检出锁 + 显式归还的常数开销），
  与轮次 9 相同的「内存换常数 CPU」权衡，幅度在用户不可感知范围。
- **双硬闸门全过**：两策略逐包 FNV-1a 滚动哈希与字节数逐拓扑全等。

生产语义折算：驻留不再随连接数增长，C 条连接的组帧缓冲稳态占用从
C × ≤256 KiB 封顶为 16 × 256 KiB = 4 MiB；连接数 128 → 1,024 时收益
从 32 MiB → 256 MiB 线性放大。

## 5. 测试与门禁

- 新增同步单测 `pooled_frame_buffer_reuse_round_trip`（复用命中+容量延续）、
  `pooled_frame_buffer_shrinks_and_respects_cap`（收缩治理+驻留上限）；
  原异步容量断言测试改写为 `write_packet_frames_correct_across_size_mix`
  （大小包混写的线上字节正确性，容量语义移交同步池测试）。
- 门禁：`papokin-world` 270 / `papokin` 407 / `papokin-protocol` 142
  （+2 新增）全绿；`cargo clippy --workspace --all-targets` 零告警。

## 6. 结论与后续

组帧侧与压缩侧（轮次 9）资源全部完成池化收编：协议编码路径的
per-connection 内存驻留至此清零，剩余与连接数相关的内存主要在业务层
（玩家实体、区块兴趣集、入站队列——均有独立水位治理）。后续轮次候选：
入站解压 `ZlibDecoder` 的冷路径分配、批路径 `written_packets` 簿记 Vec
的复用、或转向 CPU 面（组帧拷入的 `extend_from_slice` 合并）。
