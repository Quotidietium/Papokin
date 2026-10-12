# 轮次 40：NBT 复合标签容器紧凑化（HashMap → 插入序 Vec）

> 日期：2026-10-12 ｜ 主题：内存（常驻容器开销）+ CPU（顺带） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/nbt_compound_compact_map.rs](../../../benchmark/src/bin/nbt_compound_compact_map.rs)
> 数据：[round40-nbt-compound-compact-map.json](round40-nbt-compound-compact-map.json)

## 问题

`NbtCompound` 是全仓库最普遍的通用容器：每个实体（`custom_data`）、
每个区块（`custom_data`/`pending_block_entities`/实体快照
`Vec<NbtCompound>`）、每个世界（两级 `custom_data`）、方块实体 PDC
（`custom_block_entity_data: DashMap<BlockPos, NbtCompound>`）、
结构模板、刷怪笼/宝库保留 NBT、物品 `custom_data` 组件都以它
长期驻留；存档/网络/命令（SNBT、`/data`、NBT 路径）三条路径
还持续做解析态构建。旧承载是 `HashMap<Box<str>, NbtTag>`：

- 每个非空复合标签承担一张哈希表——表槽余量（12 条即 32 槽）、
  SIMD 组复制的控制字节、rehash 增长链——外加逐键独立堆分配；
- `NbtTag` 因 `Compound` 变体**内联** 48B 的哈希映射而膨胀到
  56B，所有 `Vec<NbtTag>`（列表标签、SNBT 解析中间态）按此
  付费；
- 空复合标签内联 48B——实体/区块常驻的 `custom_data` 字段即使
  从未写入也逐实例付费。

NBT 复合标签在实践中是小映射（方块实体/实体存档通常 ≤20 键），
哈希表的结构性开销在这个尺寸段是纯浪费。

## 机制

`NbtCompound.child_tags: HashMap<Box<str>, NbtTag>` →
`NbtMap`（`crates/papokin-nbt/src/map.rs`，插入序
`Vec<(Box<str>, NbtTag)>`）：

1. **兼容门面**：`NbtMap` 提供对齐 `HashMap` 常用子集的 API
   （`get`/`get_mut`/`insert`/`remove`/`contains_key`/`keys`/
   `values`/`iter`/`iter_mut`/`IntoIterator`×3/`Index`/`Extend`/
   `FromIterator`），全仓约 100 处 `child_tags` 直接调用点仅
   2 处需要改动（`papokin-command` SNBT 与 `papokin` wasm 宿主
   的 `FromIterator` 二义性，显式标注类型解决），其余零改动
   迁移。
2. **尺寸收缩**：`Compound` 变体内联从 48B 降到 24B，`NbtTag`
   56B → **32B**（`Vec` 变体重新成为最大变体），空复合标签
   内联 48B → **24B**。
3. **语义保持**（红线）：
   - 相等性手工实现为**键序无关**（对齐 `HashMap` 的
     `PartialEq` 语义，非派生）；
   - `insert` 对已存在键**原位替换值并保留首次插入位置**，
     与哈希映射「后者胜」取值语义一致（解析重复键行为不变）；
   - 迭代序从「进程内任意」（HashMap 序）固定为「插入序」：
     NBT 线上格式本就不定义键序，所有既有读者（原版/Paper/
     回读解析器）按键取值，插入序只是把任意变确定，逐字节
     往返反而更稳定；
   - `put(&str)` 走 `insert_str` 借用键路径，替换时不再白付
     键盒分配（旧实现总是先 `name.into()` 再 insert）。
4. **查找复杂度与安全论证**：`get` 为线性扫描。NBT 键是短
   字符串（memcmp 快于哈希），条目数受解析层约束（深度 ≤512、
   网络侧 `custom_data` 另有 64KiB 序列化总量上限），最坏情况
   有界、无放大；基准查找段实测短键线性扫描**快于**哈希
   （2.3-2.9×）。

连带勘误：`PreservedChunkData.fields_blob` 的注释口径更新——
轮次 24 的「解析态 2.4× 于序列化字节」系 HashMap 时代实测，
紧凑映射后该比值收窄，blob 驻留仍占优（精确贴合 + 零物化成本），
机制不变。

## 负载模型

15000 个复合标签（确定性语料，xorshift64* 可复现）：

- 5000 **实体存档形**（~25 键）：id/Air/Health/Fire/HurtTime/
  UUID IntArray/Pos/Motion/Rotation 列表 + `attributes` 子复合 +
  `ArmorItems` 4 槽（复用物品形状：id/count/damage/30% 带
  `components` 子复合）+ `Brain.memories`（0-11 键）+ Paper
  保留键（0-5）；
- 5000 **方块实体形**（~12 键）：x/y/z/id + `Items` 8 槽物品
  列表 + 1/3 带 CustomName + 1/5 带 Lock；
- 5000 **物品 `custom_data` 形**（嵌套 3 层，~30 键）：
  `components` → `minecraft:display_name`（Name + Lore 0-3 行）
  与 `minecraft:custom_data`（serial + flag_0..4）。

观测：语料净驻留、构建/查找/序列化回读/深克隆四段的分配次数
与耗时；两臂输出经各自序列化后**共用生产解析器回读**做
canonical 键序无关指纹（中立即裁判）。

## 结果（release，3 次实跑取代表值）

| 指标 | 旧臂（HashMap） | 新臂（NbtMap） | 比值 |
|------|----------------|----------------|------|
| `NbtTag` 尺寸 | 56 B | **32 B** | −43% |
| 复合标签内联（空） | 48 B | **24 B** | −50% |
| 语料净驻留 | 60 583 282 B | **35 600 310 B** | **1.70×** |
| 每复合标签均值 | 4038.9 B | 2373.4 B | −41% |
| 构建分配 | 790 490 次 | 775 337 次 | −1.9% |
| 构建耗时 | 56.8 ms | 38.5 ms | 1.48× |
| 查找耗时（45 000 次） | 2317 µs | 804 µs | **2.88×** |
| 查找分配 | 0 | 0 | 持平 |
| 序列化+回读耗时 | 109.1 ms | 93.1 ms | 1.17× |
| 深克隆耗时 | 9.3 ms | 6.9 ms | 1.34× |
| 深克隆分配 | 86 890 次 | 86 890 次 | 持平 |

驻留数字逐字节确定（三轮全同）；耗时三轮波动 <5%。

## 闸门

1. **构建内容 canonical 指纹全等**：两臂序列化字节经生产解析器
   回读后键序无关递归指纹相等（`0x1044e1bd6ef067b1`）——语义
   等价硬证明。PASS
2. **查找电池命中与值指纹全等**：命中计数 1666=1666、取值指纹
   相等。PASS
3. **驻留字节下降**：60.58 → 35.60 MB。PASS
4. **构建分配次数不增**：790 490 → 775 337。PASS

## 影响面与测试

- 改动：`papokin-nbt`（新增 `map.rs` + compound 接线 + 9 项新
  单测：键序无关相等/替换保位/移除保序/Index panic/迭代插入序/
  收集扩展/尺寸断言）、`papokin-command`（SNBT 一处）、
  `papokin`（wasm 宿主一处）、`papokin-world`（注释口径）。
- 门禁：`cargo fmt` 全仓通过；clippy（debug+release
  `--all-targets -D warnings`）0 错误；workspace 全量测试
  **1266 通过 0 失败**（papokin-nbt 19/19）。
- 行为可见差异审计：`Display`（SNBT 展示）与序列化的键序由
  进程任意序变为插入序——`/data` 命令输出前本就显式排序键；
  NBT 读者按键取值，无依赖方。Debug 格式从 map 形变 list 形，
  无断言依赖（全测试通过佐证）。

## 结论

NBT 复合标签的容器开销整体收缩 1.70×，`NbtTag` 全场景瘦身
43%，空复合标签减半——实体/区块/世界常驻的 `custom_data` 与
结构模板、物品组件全部受益；构建/查找/序列化/克隆四路径
无一回归（查找反获 2.3-2.9×）。这是「不影响语义的表示层
重写」：相等性、重复键取舍、线上格式、API 面全部保持，语义
等价由中立解析器指纹闸门与全量测试双重证明。
