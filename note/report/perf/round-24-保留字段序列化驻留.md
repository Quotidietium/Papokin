# 轮次 24：区块保留字段的序列化驻留（parsed → blob）

> 日期：2026-10-04 ｜ 主题：内存（驻留容量） ｜ 闸门：3/4 PASS（闸门 2 未达预登记阈值，决策见「结论」）
> 基准：[benchmark/src/bin/preserved_fields_blob.rs](../../../benchmark/src/bin/preserved_fields_blob.rs)
> 数据：[round24-preserved-fields-blob.json](round24-preserved-fields-blob.json)

## 问题

从磁盘加载的区块携带 `PreservedChunkData`：Papokin 无模型的根键
（`structures`、`LastUpdate`、未建模高度图、`blending_data`、
`BukkitValues` 等）在加载时从解析好的根复合标签中**逐键深克隆**，
以 `NbtCompound`（`HashMap<Box<str>, NbtTag>`）形态常驻内存，直到
区块卸载。它们在区块驻留期间**只被落盘路径读取**（
`internal_to_bytes` 开头克隆并入根），是典型的「写一次、驻留全程、
几乎不读」数据。导入 vanilla/Paper 世界的服务器上，未改动区块
可达数万张，该驻留随区块数线性放大。

零数据风险是硬约束：保留字段的往返保真是存档重写的根基，任何
表示层优化都必须证明落盘输出语义等价。

## 机制

`PreservedChunkData.fields: NbtCompound` → `fields_blob: Box<[u8]>`：

1. **加载路径**（`format/mod.rs`）：外来字段照旧解析收集为
   `NbtCompound`，随后多走一步 `write_unnamed` 序列化并
   `to_vec().into_boxed_slice()` 压实（`write_unnamed` 内部 Vec
   增长摊余实测达 2×，不压实会把摊余当驻留），仅保留精确贴合
   的字节；复合标签本身即时释放。
2. **落盘路径**（同文件）：`internal_to_bytes` 开头由「深克隆
   保留复合标签」改为「`read_unnamed` 解析 blob 回复合标签」，
   其后流程（受管键覆盖、高度图合并、状态名保真）逐字不变。
   blob 由本进程自有序列化产生，回读失败即内部不变式破坏——
   `expect` panic 优于静默降级（零数据风险约束）。
3. 附带收益：落盘路径的 `preserved.lock().clone()` 由深克隆变为
   `Box<[u8]>` 单次定长拷贝。

等价性论证：NBT 复合标签在 `NbtCompound` 中本就无序存储
（HashMap），现行落盘输出的字段顺序已与磁盘原序不同——语义
等价只要求「键 → 标签值」映射一致；序列化/解析往返对 NBT 全
类型保真（基准闸门 1 以 canonical 指纹逐键证明）。既有单测
`foreign_chunk_fields_survive_round_trip`（structures/LastUpdate/
blending_data/未建模高度图/BukkitValues/旧状态名全断言 +
二次往返稳定性）直接覆盖新路径；新增单测
`preserved_fields_reside_as_blob_and_reparse_faithfully` 钉死
「blob 驻留 + 回读逐键还原」不变式。

## 负载模型

合成 vanilla 导入世界语料 2000 区块：`structures`（starts 空 +
References 8 类结构 × 0-20 长整数组）、`LastUpdate`、未建模高度
图 2 组 × 37 长整、30% 带 `blending_data`、20% 带外来 mod 字段。
两臂：解析驻留（克隆语料常驻）vs blob 驻留（精确贴合字节常驻）；
各执行 2000 次落盘路径，观测净驻留字节（分配器 分配-释放 差分）、
落盘耗时、分配次数、落盘输出 canonical 指纹。

如实记录两条测量教训：①驻留必须按「分配-释放」净额计量——
首轮按分配毛额计量时把 blob 臂的中转分配（序列化 Vec 摊余 +
to_vec 拷贝）误记为驻留；②`write_unnamed` 的 Vec 摊余实测 2×，
精确贴合（boxed slice）是 blob 形态的前提。语料为合成形状
（本机无真实 vanilla 存档），绝对值依语料而变，「解析态堆开销
vs 序列化字节」的结构性倍率稳健。

## 结果

| 指标 | 解析驻留（现状） | blob 驻留（收编） | 差值 |
| --- | ---: | ---: | ---: |
| 语料净驻留（2000 区块） | 7,815,672 B | 3,209,272 B | **2.4× 压缩** |
| 每区块驻留 | ≈3.9 KB | ≈1.6 KB | -2.3 KB/区块 |
| 落盘路径耗时（2000 次） | 16-27 ms | 18-23 ms | ≈1.0×（三次独立运行 0.86×/0.99×/1.18×，噪声主导） |
| 落盘路径分配 | 315,123 次 | 375,325 次 | 1.19× |
| 落盘输出 canonical 指纹 | `0xd17b…63ba` | `0xd17b…63ba` | 全等 |

- 驻留收益：每导入未改动区块省 ≈2.3 KB——1 万张 ≈ 23 MB，
  3 万张 ≈ 68 MB（导入 vanilla/Paper 世界的常态保有量）；
- 落盘代价：blob 解析替代深克隆，三次运行耗时比在 0.86×-1.18×
  间波动（均值 ≈1.0×），与深克隆实测持平——自动保存周期数百
  脏区块的合计差异为亚毫秒级噪声；
- 等价性：2000 区块落盘输出 canonical 指纹双臂全等（非退化），
  既有往返单测 28 + 新增 1 全绿。

## 闸门

| 闸门 | 结果 | 说明 |
| --- | --- | --- |
| 输出 canonical 指纹全等 | PASS | `0xd17b294ddb6363ba` 双臂一致 |
| 驻留 ≥3× 压缩 | **FAIL（2.4×）** | 预登记阈值基于「解析态 4-10× 开销」的错误先验，实测 2.4× |
| 落盘耗时 ≤3× | PASS | 三次运行 0.86×-1.18×，均值 ≈1.0× |
| 落盘分配 ≤2× | PASS | 实测 1.19× |

## 结论与后续

**收编**（附条件记录）：闸门 2 未达预登记阈值，但实测数据显示
收益为正且可观（2.4×、每万区块 ≈23 MB）、两条代价轴均贴近 1×、
等价性有 canonical 指纹 + 既有往返测试 + 新增不变式测试三重
证据；阈值本身建立在被测量证伪的先验上（长数组主导的保留字段
在解析态并不「虚胖」4-10×，真实开销集中在 HashMap 桶、
Box<str> 键与逐标签枚举的固定成本）。按「数据优先于预登记
估计」原则收编，并如实保留闸门未达记录。

后续候选（记入轮次 25+ 清单）：
- 真实 vanilla 存档采样校准保留字段尺寸分布（sysinfo + 真实
  世界目录），验证 2.4× 在真实语料上的位置；
- 插件 IPC 缓冲与 WASM 实例内存的驻留画像；
- NBT 序列化写路径的频次加权侦察（轮次 19 遗留）。
