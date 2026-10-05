# 轮次 19：区块批次发送缓冲驻留与脏元数据版本集内联

> 日期：2026-10-05 ｜ 主题：内存 + CPU（玩家区块流发送与实体元数据扇出分配流失）
> 基线：轮次 18 收官（0.3.29）；本轮改动落库后以 0.3.30 发布
> 基准程序：[benchmark/src/bin/chunk_batch_reuse.rs](../../../benchmark/src/bin/chunk_batch_reuse.rs) ｜ 原始数据：[round19-chunk-batch-meta-reuse.json](round19-chunk-batch-meta-reuse.json)

## 1. 优化点与动机（生产可达性已验证）

延续「先验证生产可达，再落库」纪律。两处发送路径残余分配面：

**区块批次发送**（`ChunkSender`）。移动突发期每玩家每 tick 一批，
每批新建五个 Vec：① 候选 `Vec<PreparedChunk>`（
`collect_sorted_candidates` 带配额容量新建）；② 小 pending 排序
Vec（pending ≤ 16 时的排序分支）；③ 并行编码结果 Vec（rayon
`collect`）；④ 输出 `Vec::with_capacity`；⑤ 已派发位置
`Vec::with_capacity`（`commit_batch` 返回）。玩家传送/加入/跨图
时区块流式发送持续数十 tick；正常行走跨区块边界同样逐 tick
触发批次。

**脏实体元数据**（`send_dirty_entity_data`，每脏实体每 tick）与
**全量元数据**（`send_meta_data`，生成/刷新/状态切换）。每次调用
新建：① 收件人 `Vec<&Player>`（逐级扩容 3 次分配）；②
`BTreeMap<版本, Vec>` 分组与每版本 Vec（节点 + 逐级扩容）；
③ 版本列表 Vec（仅供 `pack_dirty_for_versions` 取版本集）。
水下生物/玩家空气值逐 tick 变脏，潜行/游泳/着火/发光等状态
切换均走此路径。

## 2. 改动内容

### ChunkSender 批次五缓冲驻留（`net/chunk_sender.rs` + `entity/player.rs`）

- `PreparedBatch` 改借用形态 `PreparedBatch<'a> { chunks: &'a
  [PreparedChunk], .. }`；`prepare_batch` 新增 `&mut Vec<PreparedChunk>`
  出参重填（原带配额容量新建）。
- 小 pending 排序 Vec 收编为 `ChunkSender::sort_scratch` 字段
  （`prepare_batch` 持 `&mut self` 锁内使用，无跨锁借用）。
- `encode_batch` 改双出参：编码结果经 rayon `collect_into_vec`
  复用容量收集，再重填输出 Vec（原 `collect` + `with_capacity`
  双分配）。
- `commit_batch` 改 `&mut Vec<Vector2<i32>>` 出参（原
  `Vec::with_capacity` 返回）；epoch 失配/空编码时重填结果为空，
  与原返回空 Vec 语义一致。
- 玩家侧新增 `Player::chunk_batch_scratch:
  Mutex<ChunkBatchScratch>`（候选/编码结果/输出/派发四缓冲）：
  `Player::tick` 单线程执行（逐玩家每 tick 恰好一次），持锁横跨
  「准备→并行编码→提交」全程无竞争；批次借用与编码出参为
  暂存内互不相交字段借用。
- `chunk_sender` 测试模块同 commit 适配：`batch_of` 改借用、
  编码/暂存由局部 Vec 充当；`prune_sweeps` 测试的「释放强引用」
  语义改为先结束借用再 drop 持有 Vec。

### 元数据版本集栈内联（`entity/mod.rs` 两站点）

`send_meta_data` 与 `send_dirty_entity_data` 改两趟内联：第一趟以
栈上 4 槽收集在线协议版本集；第二趟逐版本打包并重新过滤收件人
直发（版本种数为 1 的常态下，收件人 Vec、BTreeMap 与版本 Vec
全消）。超过 4 种版本时回退原分配路径（行为完全保持；单机 4+
版本已属异常配置）。`send_dirty_entity_data` 保持「无收件人不
调用 `pack_dirty_for_versions`」的清脏语义（版本集为空即返回）。

## 3. 兼容性论证（红线 2）

- **发送序列**：批次内区块顺序、编码字节、派发位置列表逐元素
  不变（`collect_into_vec` 与 `collect` 同为保序收集）；配额、
  epoch 校验、`CChunkBatchStart/End` 时序不变。
- **元数据投递**：两趟过滤与原「先收集后按版本分组」产生相同
  的（收件人, 字节）对；每个客户端仍恰好收到其版本一份包，
  跨版本顺序本来不可观测；版本槽溢出回退保证任意版本混合下
  行为不变。
- **清脏语义**：`pack_dirty_for_versions` 的调用时机与入参
  （在线版本集切片）不变；无收件人时不打包不清脏。
- **锁纪律**：`Player::tick` 逐玩家单线程，`chunk_batch_scratch`
  无竞争路径；`ChunkSender` 锁内只用自有 `sort_scratch`，编码仍
  在锁外并行（与原文一致）。

## 4. 基准结果

模型：600 tick ×（200 流式玩家 × 每 tick 一批（8-24 区块）+
150 脏实体 × 10 收件人）；语义负载（每区块编码字节约 3200 次
分配/tick、每版本脏项字节约 165 次/tick）两形态同源、对账抵消。

| 形态 | 总分配 | 总字节 | 每 tick 分配 | 每 tick 字节 |
|---|---|---|---|---|
| 逐调用新建 | 3,268,001 次 | 434,694,994 B（414.6 MiB） | 5446.7 次 | 724,492 B |
| 驻留+内联 | 2,019,468 次 | 154,777,306 B（147.6 MiB） | 3365.8 次 | 257,962 B |

**纯结构分配每 tick 约 2081 次 → 0**（复用形态剩余的 3365.8
次/tick 全部是语义负载，两形态同源）；总量 -38.2% 分配次数、
-64.4% 字节。

硬闸门：编码块 + 派发位置 + 元数据投递指纹两形态全等
（`内容门=true`）；复用形态稳态分配严格小于逐调用新建。

## 5. 测试与门禁

- `cargo test -p papokin-world --lib`：270 通过
- `cargo test -p papokin --lib`：407 通过（含 `chunk_sender` 测试
  模块 4 例适配后全绿）
- `cargo test -p papokin-protocol --lib`：142 通过
- `cargo clippy --workspace --all-targets`：0 错误
- 基准内容门/分配门全过

## 6. 结论

发送路径最后两块常态结构分配面收编：区块批次五缓冲驻留玩家侧
（移动突发期每玩家每 tick 省 4-5 次分配），元数据版本集栈内联
（每脏实体每次广播省 5-8 次分配）。语义负载（区块编码字节、
脏项打包字节）保持逐字节不变。

至此玩家/实体/世界三侧的发送与查询路径常态结构分配基本肃清，
剩余已知项均为安全保留（`update_player_position` 快照防死锁）
或逐事件语义分配（包构造、插件事件对象、NBT 序列化）。后续
轮次转向：逐事件路径的**频率加权**排查（NBT 序列化缓冲、
`Bytes` 出站链、命令/聊天文本）与驻留结构内存审计（大容量
HashMap/缓存的容量收缩策略）。
