# 轮次 21：区块编码缓存逐出后的桶容量回收

> 日期：2026-10-04 ｜ 主题：内存（驻留容量） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/encode_cache_bucket_reclaim.rs](../../../benchmark/src/bin/encode_cache_bucket_reclaim.rs)
> 数据：[round21-encode-cache-bucket-reclaim.json](round21-encode-cache-bucket-reclaim.json)

## 问题

`SharedChunkEncodeCache`（按世界共享的区块编码缓存，预算 64 MB）的
`prune_if_over_budget` 只 `remove` 条目：`DashMap` 的桶数组在移除后
**永不收缩**。探索型玩家走过数万区块后，对应规模的桶数组（每槽约
115 B：104 B 键值对 + 控制字节 + 1/8 空载）永久驻留——即使条目
早已被逐出。生产侧死条目清扫也只在下次超预算 prune 时顺带发生，
桶容量在收编前事实上「只增不缩」。预算封顶（64 MB 负载 ≈ 1.6-6.4
千条目）时单表桶驻留约 0.5-0.9 MB，大规模卸载后本应回落到几十 KB。

附带问题：预算记账 `encoded_bytes()` 只算线上负载（payload +
light_payload），不含条目结构与键的固有开销（约 100 B/条）；轻区块
占多数时条目数可达 1-3 万，口径偏差可达预算的 5%-8%。

## 机制

1. **逐出后桶容量回收**（`chunk_sender.rs` `prune_if_over_budget`
   尾部）：距离逐出循环结束、CAS 守卫释放之前插入 shrink 判定——
   `evicted > 0 && map.capacity() > map.len() × 4 + 64` 时
   `map.shrink_to_fit()`。安全性三点论证（侦察确认）：
   - 插入点不持有任何 `DashMap` 分片守卫（两个 `iter()` 的读守卫
     已在 `collect` 点释放），`shrink_to_fit(&self)` 签名兼容；
   - CAS 仍在持有，不会与另一个 prune 并发 shrink；
   - dashmap 6.2.1 的 `shrink_to_fit` **逐分片顺序取写锁**（非同时
     锁全部），并发 `get_fresh`/`insert` 只在命中同一分片时短暂
     park，单分片重哈希为微秒级，远低于 tick 预算；仓内已有同款
     先例（`level.rs::clean_memory` 对 10 万级条目的 `loaded_chunks`
     周期 shrink）。
   - 4× 滞回（与 `papokin-util::capacity` 同参数）防止探索-回流
     边界的反复扩缩。
2. **预算记账口径补结构开销**（`encoded_bytes()`）：单点加入
   `size_of::<EncodedChunk>() + size_of::<键>()` 常量（所有记账/销账
   路径同经此函数，口径自动一致）。

## 静态注册表探查（否定结论，如实记录）

本轮同时侦察了「静态注册表/全局表容量压实」方向（`TRANSLATIONS`、
`PLACED/CONFIGURED_FEATURES` 等运行期构建的全局 `HashMap`），
结论为**不实施**：hashbrown/std `HashMap` 的桶数按 2 幂增长且
负载因子 7/8，纯插入填充的 map 容量**天然等于** `shrink_to_fit`
的目标值——收缩是空操作。大表（block/biome/registry/recipes）
本来就是编译期 const/phf 数据，零堆容量浪费。Vec 侧仅
`OVERWORLD_FEATURES_PER_STEP` 内层有约 2-4 KB 摊倍余量，量级
不值得立项。记录此结论以免后续轮次重复探查。

## 负载模型

对齐真实「探索 → 停探卸载 → 再探索」节奏：
1. 探索期 8 周期 × 2000 条新编码条目（1.6 万条，桶数组冲至峰值）；
2. 停探卸载：90% 条目标记弱引用死亡，触发逐出清扫（生产侧在下次
   超预算 prune 时顺带，模型等效直接触发）；
3. 再探索期 4 周期 × 2000 条（检验桶回涨摊开销）。

两臂：无收缩（现状 `remove`-only）vs 收缩（滞回 shrink）。条目模型
为 104 B 键值对（对齐生产 `EncodedChunk` 尺寸），负载字节以 24 KB
权重记账驱动预算逐出。

## 结果

| 指标 | 无收缩 | 收缩 | 差值 |
| --- | ---: | ---: | ---: |
| 卸载后桶驻留 | 2,953,080 B | 186,368 B | **15.8× 回收** |
| 收尾桶驻留（再探索后） | 2,961,400 B | 1,490,944 B | 2.0× |
| 全程分配次数 | 22 | 26 | +4 |
| 收缩事件 | 0 | 1 | 单次集中波次 |
| 存活指纹（卸载后/收尾） | — | — | 双臂全等（`0xb5df…51c3` / `0xf15c…b947`） |

- 卸载瞬间：收缩臂把 1.6 万条目峰值的桶数组（~2.95 MB）直接回落到
  1.6 千存活条目的适配容量（~0.19 MB），回收 15.8×；
- 再探索期：桶数组随插入自然回涨至当前负载适配值（1.49 MB vs 无收缩臂
  的 2.96 MB 历史峰值），且**未触发**第二次收缩（evicted=0 保护 +
  4× 滞回均生效），全程无扩缩振荡；
- 代价：全程仅多 4 次分配（1 次 shrink 重哈希 + 回涨路径的微弱差）。

## 闸门

| 闸门 | 结果 | 说明 |
| --- | --- | --- |
| 存活指纹全等（双采样） | PASS | 逐出/收缩不改变存活条目集合与内容 |
| 卸载后桶驻留 ≥4× 回收 | PASS | 实测 15.8× |
| 收缩事件 ≤ 2 | PASS | 实测 1（仅卸载波次，再探索期零振荡） |
| 额外分配 ≤ 16 | PASS | 实测 +4 |

## 结论与后续

收编：桶容量回收以 4 次额外分配的代价换来卸载后 15.8× 的桶驻留回落，
语义零变化（指纹全等、既有 6 项单测全绿 + 新增 2 项收缩行为单测）。
记账口径补齐条目结构开销，64 MB 预算现在同时覆盖负载与驻留结构。

后续候选（记入轮次 22+ 清单）：
- `seen_by` 等事件驱动型 `DashSet` 的周期压实（需结构改造或随 prune 顺带）；
- 线程局部分片暂存（thread_local box scratch）的跨线程峰值审计；
- 真实服务器负载下的驻留采样（sysinfo 已在 benchmark 依赖内）。
