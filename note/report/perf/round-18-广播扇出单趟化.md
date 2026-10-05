# 轮次 18：广播扇出单趟化与方块冲刷驻留交换

> 日期：2026-10-05 ｜ 主题：内存 + CPU（世界→客户端包扇出路径分配流失）
> 基线：轮次 17 收官（0.3.28）；本轮改动落库后以 0.3.29 发布
> 基准程序：[benchmark/src/bin/broadcast_flush_reuse.rs](../../../benchmark/src/bin/broadcast_flush_reuse.rs) ｜ 原始数据：[round18-broadcast-flush-reuse.json](round18-broadcast-flush-reuse.json)

## 1. 优化点与动机（生产可达性已验证）

延续「先验证生产可达，再落库」纪律。世界→客户端的包扇出路径上
有两类纯瞬态结构每调用/每 tick 重建：

**广播分组**。每次广播经 `collect_java_recipients_by_version`
新建 `BTreeMap<协议版本, Vec<&JavaClient>>` + 每版本一个 Vec，
唯一目的是让同版本收件人共享一份序列化字节。全仓 121 处广播
调用点；每 tick 无条件或高频执行的热点包括：

- **`TrackedEntity::send_to_tracking_players` 一族**（每个移动
  实体每 tick 至少一次：移动/旋转/属性/移除包，300 移动实体即
  600+ 次/tick）；
- **`flush_block_updates` 的分节广播**（每变更节每 tick 一次）与
  **`flush_synced_block_events`**（活塞等每事件一次）；
- **声音/粒子/游戏事件**（`play_sound_raw` 等，每发声一次）。

**冲刷缓冲**。`flush_block_updates` 每 tick `mem::take` 取走变更
图（容量随之丢弃，下一 tick 生产者推入时逐级重分配）并新建分节
`HashMap<节, Vec<变更>>` + 每节一个 Vec；`flush_synced_block_events`
同样 `mem::take` 丢弃事件队列容量。两函数每世界每 tick 无条件
执行（正常模式在 `World::tick` 内、冻结模式在
`tick_players_and_network` 内，另有 setblock/fill/clone/place
等命令与插件结构放置路径同步调用）。

## 2. 改动内容

### 广播核心：单趟内联版本槽（`world/mod.rs`）

`broadcast_java_clients` 重写为单趟懒序列化：栈上 4 槽
`[Option<(JavaMinecraftVersion, Option<Bytes>)>; 4]` 版本缓存，
逐收件人查槽——命中共享字节直发；空槽序列化一份填入再发；数据
槽为 `None` 标记该版本序列化失败（不支持/写错误），后续同版本
收件人直接跳过（与原先 `continue` 语义一致）。超过 4 种协议版本
同播时，超出的收件人退化为逐个序列化直发（投递语义不变，仅失
同版本共享；单机 4+ 版本已属异常配置）。

- 绝大多数服务器全员同一协议版本，首槽即命中，分组结构零分配；
- 语义负载（每版本一份序列化字节，逃逸进各客户端出站队列）
  保持不变，两形态对账相互抵消；
- 错误日志仍在每个版本首次失败时输出一次，频率与原文一致。

### 调用点迁移（10 处机械站点 + 3 处实体追踪热点）

原「`collect_java_recipients_by_version` → `broadcast_java_grouped`」
两步合并为一行 `broadcast_java_clients(packet, iter.map(|p| &*p.client))`：

- `world/mod.rs`：`broadcast_packet_all`、`broadcast_packet_except`、
  `play_sound_raw`、`play_sound_raw_expect`、`broadcast_to_chunk`、
  `broadcast_to_chunk_except`、`flush_block_updates` 多变更分支；
- `entity_tracker.rs`：`broadcast_removed`、
  `send_to_tracking_players`、`send_to_tracking_players_filtered`。

`broadcast_java_grouped` 随之零调用，删除。
`collect_java_recipients_by_version` 保留给 3 处**包内容本身随版本
变化**的站点（实体元数据 ×2、皮肤层广播——它们需要版本分组来
逐版本构造不同字节，非 serialize-once 场景）。

### 冲刷驻留交换（`world/mod.rs`）

新增 `World::block_update_flush_scratch: Mutex<BlockUpdateFlushScratch>`
（变更交换图 + 分节分组图）与 `block_event_flush_buffer:
Mutex<Vec<BlockEvent>>`：

- `flush_block_updates`：`swap` 接替 `mem::take`——生产者队列
  容量跨 tick 保留，消除逐级重分配；分节分组图驻留，每 tick
  清值留桶再重填；分组图条数超 4096 时整图清空兜底（防长跑
  服务器离散节键无限累积）。
- `flush_synced_block_events`：事件队列同样 `swap` 接替
  `mem::take`。

**两把锁而非一把**（死锁论证）：事件冲刷循环经活塞处理器可触达
插件回调，插件可经结构放置等 WASM API 同步重入
`flush_block_updates`——共用一把 `Mutex` 会互锁，故变更冲刷与
事件冲刷各持独立暂存锁。变更冲刷循环内仅做出站入队与 dashmap
读、不触达插件（`broadcast_to_chunk` 纯入队），其持锁期间无
重入路径；事件冲刷无插件/API 调用方（仅 tick 路径），持锁迭代
安全。冲刷期间新入队的变更/事件仍下一 tick 处理，与 `mem::take`
语义完全一致。

## 3. 兼容性论证（红线 2）

- **投递语义**：每个目标客户端收到的字节序列逐包不变（同版本
  共享同一份 `Bytes`，引用计数克隆入队）；跨客户端投递顺序
  本来各自独立、不可观测。
- **失败语义**：版本不支持跳过、序列化错误记录后跳过，均与
  原文逐版本一致；槽满兜底仅失去共享优化，不丢投递。
- **冲刷语义**：`swap` 与 `mem::take` 对「冲刷期间新入队者下一
  tick 处理」的语义完全一致；分节广播集合与单/多变更分支
  判定不变。
- **API 面**：`broadcast_java_clients` 签名不变（server/mod.rs
  现有调用自动受益）；仅删除私有辅助 `broadcast_java_grouped`。

## 4. 基准结果

模型：600 tick ×（940 次广播 × 12 收件人 + 400 方块变更/60 节
+ 80 方块事件）每 tick；语义负载（每版本一份字节，1034 次
分配/tick）两形态同源、对账抵消。

| 形态 | 总分配 | 总字节 | 每 tick 分配 | 每 tick 字节 |
|---|---|---|---|---|
| 逐调用新建 | 3,016,800 次 | 461,442,600 B（440.0 MiB） | 5028.0 次 | 769,071 B |
| 单趟+驻留 | 620,400 次 | 26,987,400 B（25.7 MiB） | 1034.0 次 | 44,979 B |

**纯结构分配每 tick 3994 次 → 0**（复用形态剩余的 1034 次/tick
全部是语义负载，两形态同源）；总量 -79.4% 分配次数、-94.2%
字节。

硬闸门：投递多重集指纹（交换律混合）两形态全等（`内容门=true`）；
复用形态稳态分配严格小于逐调用新建。

## 5. 测试与门禁

- `cargo test -p papokin-world --lib`：270 通过
- `cargo test -p papokin --lib`：407 通过
- `cargo test -p papokin-protocol --lib`：142 通过
- `cargo clippy --workspace --all-targets`：0 错误
- 基准内容门/分配门全过

## 6. 结论

世界→客户端包扇出路径的常态结构分配面肃清：广播分组
（BTreeMap+Vec，每广播 2-3 次堆分配）与方块冲刷缓冲（每 tick
重建分组图 + 队列容量流失）合计每 tick 约 4000 次结构分配归零，
语义负载保持逐字节不变。

下一轮候选：① `send_dirty_entity_data` / 实体元数据等**版本内容
相关**站点的驻留化（包字节本身随版本构造，需另一套版本集合
内联方案）；② `ChunkSender` 批次发送缓冲复用（候选/排序/编码
结果/已发列表五 Vec 每批次新建，移动突发期连续触发）；③
chunker 差集计算的 `loading_chunks.clone()`。
