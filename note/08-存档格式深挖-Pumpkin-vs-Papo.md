# 08 · 存档格式深挖：Pumpkin vs Papo（磁盘字节层对比）

> 生成时间：2026-09-19 · 前置：[07-存档系统对比](07-存档系统对比-Pumpkin-vs-Papo.md)（管线/时机/语义层）
> 本文只谈**磁盘上的字节**：容器格式、压缩、NBT 载荷字段、各数据文件结构、互操作性。
> 引用约定同 07：Pumpkin 侧为仓库相对路径真实行号；Papo 侧 **[0001]** 指 `REF/Papo-Java-0.80.0-src/paper-server/patches/features/0001-Moonrise-optimisation-patches.patch`（行号为补丁内行号），`*.patch` 为 `patches/sources/` 下补丁文件行号，`.java` 为直提交源码行号。

## 0. 一句话结论

**Papo：一种格式（vanilla MCA）做到字节级极致兼容**——默认 ZLIB-6 与 vanilla 逐字节一致，超大块走 `.mcc`，DFU 保证任意历史版本可读。
**Pumpkin：三种格式并存，anvil 是"vanilla 子集 + 私有变体"**——默认压缩 LZ4-6（非 vanilla 传统 ZLIB）、不写 structures、超大块无保护（位置表会静默溢出）；Linear V2 / Pump 是自研性能/实验路线，生态内无第二个读者。

## 1. 磁盘布局与文件命名

```
Pumpkin 世界根/                              Papo 世界根/
├── level.dat          (gzip NBT, tmp+rename) ├── level.dat / level.dat_old
├── pumpkin_custom_data.nbt (世界级 PDC)       ├── worldgen settings 等 vanilla 文件
├── dimensions/<ns>/<name>/     ← 26.2 规范    ├── dimensions/<ns>/<name>/ 或 legacy
│   ├── region/                                   ├── region/   r.<rx>.<rz>.mca
│   │   ├── r.<rx>.<rz>.mca    (anvil)            ├── entities/ r.<rx>.<rz>.mca
│   │   ├── r.<rx>.<rz>.linear (Linear V2)        │   └── *.mcc (超大块外部文件)
│   │   └── r.<rx>.<rz>.pump   (Pump)             └── poi/      r.<rx>.<rz>.mca
│   ├── entities/      ← 与 region 同格式选择
│   └── poi/           ← 永远手写 MCA，与格式配置无关
├── players/data/<uuid>.dat  (gzip NBT)      ├── players/<uuid>.dat (+.dat_old)
└── (legacy 回退: 根 region/、DIM-1/、DIM1/)  ├── stats/<uuid>.json
                                             └── advancements/<uuid>/*.json
```

- Pumpkin 维度目录解析带 legacy 回退（`level.rs:157-182`：26.2 canonical `dimensions/<ns>/<name>` → 根 `region`/`DIM-1`/`DIM1`），Papo 继承 vanilla 同款逻辑。
- **格式选择是每世界一个**（`pumpkin.toml [world.chunk] type = anvil|linear|pump`，`level.rs:251-264`），区块与实体区块共用同一选择各自实例化 saver（`level.rs:60-67`）；**POI 永远走手写 MCA**（`poi/mod.rs`），不受配置影响。
- 命名细节：anvil 键 `./r.{rx}.{rz}.mca`（`anvil.rs` get_chunk_key）、Linear 键 `./r.{rx}.{rz}.linear`（`linear.rs:386-389`）、Pump 键 `r.{rx}.{rz}.pump`（无 `./` 前缀，`pump.rs:44-48`）；Papo 全部 vanilla 式 `r.<rx>.<rz>.mca`（`RegionFileStorage.java.patch:20`）。
- Pumpkin 独有世界级文件：`pumpkin_custom_data.nbt`（世界 PDC，`world/mod.rs:7097-7104`）；Papo 统计系统完整（stats JSON），Pumpkin 未实现统计。

## 2. MCA/Anvil 容器逐字节对照

两侧同源（都遵循 vanilla Anvil 规范）：**8 KiB 头** = 4 KiB 位置表 + 4 KiB 时间戳表（每项 u32，big-endian）；**4 KiB 扇区**；**每块记录** = `u32 长度（含 1 字节压缩标识）+ u8 压缩版本 + 载荷`；位置项 = `(扇区偏移 << 8) | 扇区数`（Pumpkin `anvil.rs:381,463-464,565-586`；Papo/vanilla `RegionFile.java.patch` 上下文）。差异在**读写策略与边界处理**：

| 维度 | Pumpkin | Papo |
|---|---|---|
| 文件打开方式 | **整文件读入内存**（`file_manager.rs:99` `tokio::fs::read`），缓存的是解析后的内存结构 | FileChannel 常驻句柄（LRU 256，`misc.region-file-cache-size`，[0001]:32932-32950），按扇区 read/write |
| 写策略 | 三态 `WriteAction::{Pass, All, Parts}`（`anvil.rs:79`，write() 入口）：无脏块跳过 / **tmp 文件全量重建 + rename**（write_all）/ 局部写 | ChunkBuffer 定位扇区**就地或追加**写 + 更新 8 KiB 头（[0001]:32845-32860）；无全量重写路径 |
| 局部写碎片管理 | `write_in_place=true` 时：同扇区数就地覆盖；放不下则从尾部回扫 best-fit 换位，**超过 64 次换位放弃、退化为全量重写**（`anvil.rs:667-700` 注释自述"估算值，换位越多脏关停损坏概率越高"） | 无碎片整理（vanilla 追加式，外碎片随时间增长——Moonrise 未改放置算法） |
| 原子性 | 全量写 tmp+rename 原子；**局部写直接改原文件非原子**（脏关停时被换位的块有损坏窗口，代码注释自认） | 非原子（vanilla 同病），但有 dsync 与 flush-regions-on-save 缓解（feature 0022） |
| **超大块（>255 扇区 ≈ >1 MiB 压缩后）** | **无保护**：`sector_count()` 无上限断言（`anvil.rs:243-246`），`(offset<<8)\|count` 直接溢出位置表写坏邻居项；读侧只取低 8 位（`anvil.rs:568`），读不了 Spigot/Paper 扩展格式的大区块；**不支持 `.mcc` 外部块文件** | Spigot 255 扇区扩展读取（`RegionFile.java.patch:13-23`）+ 超 1 MiB 写外部 `.mcc` + 失败上报 ServerExceptionEvent（:49-60；features 0004/0009） |
| 压缩版本字节识别 | 1 GZip / 2 ZLib / 3 无压缩 / 4 LZ4 / 64 Custom（`anvil.rs from_byte`）；**Custom 只能识别不能解压**（`anvil.rs:147` 返回 UnknownCompression） | 同一套值（vanilla `RegionFileVersion`），四种全可用，另配 NONE |
| 头损坏容错 | 头不足 8 KiB → `InvalidHeader`（`anvil.rs` read）；块坐标与请求不符 → 报错拒载（`format/mod.rs:194-199`） | **region 头损坏时重算 header** 而非搬文件（feature 0019）；序列化异常不落盘保旧版（feature 0017） |
| 时间戳 | epoch 秒，每块维护（`anvil.rs:620-623`） | 同 vanilla |

**超大块风险只在 Pumpkin 一侧存在**，且实体区块同样用 `AnvilChunkFile`（`level.rs:63-67`）——一个塞了几百个实体 NBT 的实体区块压缩后超 1 MiB 即可触发位置表溢出。

## 3. 压缩细节

| | Pumpkin | Papo |
|---|---|---|
| 默认算法 | **LZ4，level 6**（`pumpkin-config/src/chunk.rs` `Default for ChunkCompression`）——非 vanilla 传统 ZLib；1.21+（24w04a 起）原版客户端/服务端可读 | **ZLIB，level 6** = 与 vanilla 输出**逐字节一致**（`GlobalConfiguration.java:284-290`） |
| 可配置项 | `algorithm: GZip\|ZLib\|LZ4\|Custom` + `level: u32` | `compression-format: GZIP\|ZLIB\|LZ4\|NONE` + `compression-level`（1=BEST_SPEED） |
| 块级压缩继承 | **沿用磁盘上已有块的压缩类型**重压缩（`anvil.rs:625-628` "Default to the compression type read from the file"）——转档时保持世界内一致性 | 全局配置决定；级别默认 6 保字节等价 |
| 实现优化 | flate2 / lz4-java-wrc 直接调用 | Deflater/Inflater ThreadLocal 池化 + 写缓冲 8K/32K（features 0066/0129；`PapoDeflaterOutputStream` close 时 `end()` 防原生泄漏） |
| 容器内压缩（Linear/Pump） | **zstd**（ruzstd，`CompressionLevel::Fastest`）——MCA 体系之外 | 不适用（无此格式） |

注意：Pumpkin 选 LZ4 为默认意味着其产出的 `.mca` 与典型 vanilla/Paper 世界**字节不同**（同数据不同压缩），但语义可读；Papo 的字节等价红线是为了"复制区块文件做 diff/回滚"类运维工具的确定性。

## 4. Linear V2（Pumpkin 独有）

文件级布局（`linear.rs`，规范蓝本为 Aaron2550 gist，`linear.rs:22`）：

```
┌──────────────────────────────────────────────┐
│ Superblock 26B:  u64 magic 0xC3FF13183CCA9D9A │  头尾各一份 magic（footer 8B 校验）
│                 u8 version=2 · u64 newest_ts │
│                 u8 grid_size · i32 rx · i32 rz│
│ Chunk 存在位图 128B（1024 bit）                │  ← 读侧不信任，仅参考（:474-479）
│ NBT features 字典（变长，0x00 结尾）           │  ← 恒写空（:427）
│ 桶表 每桶 13B: u32 压缩尺寸 · i8 级别=1 ·     │
│               u64 xxhash64(压缩数据)          │
│ 桶数据 ×grid_size²: zstd(桶内原始字节流)       │
│ footer: 8B magic                              │
└──────────────────────────────────────────────┘
桶内每块记录: u32 尺寸(0=缺席) + u64 时间戳 + NBT 载荷（:234-241）
```

- **分桶设计**：默认 2×2=4 桶（`linear.rs:28-30`），每桶含 256 块——目的是降低单块修改的重压缩/重写放大（代价：整文件仍需重写，因为桶布局是连续的）。
- **完整性**：每桶 xxhash64 校验，**失败跳桶继续**（其余桶仍可读，`linear.rs:519-529`）+ footer magic 校验（:503-509）——比 MCA 的"无校验"强。
- **原子写**：tmp + rename（`linear.rs:441-451`），崩溃不产生半写文件。
- 读侧整文件载入内存后逐桶解压（zstd），`get_chunks` 用 rayon 并行解块（`linear.rs:602-614`）。
- **生态孤岛**：规范蓝本是 gist 而非 LinearRegionFile 官方实现，且桶内 per-chunk timestamp 字段是自研扩展（官方 v2 桶内无独立时间戳）；Papo/原版/Paper 均无 Linear 支持。世界一旦选用 Linear 即只能 Pumpkin 自读。

## 5. Pump（Pumpkin 实验格式）

- 容器：**unnamed Java 大端 NBT** `{ x: Int, z: Int, chunks: { "<块索引十进制字符串>": ByteArray } }`（`pump.rs:54-71`）；每块载荷 = **zstd Fastest(压缩的块 NBT)**（`pump.rs:114-119`）。
- 无时间戳、无校验和、无扇区概念——纯"每区域一个 NBT 大袋子"。
- **非原子写**：`tokio::fs::write` 直接覆盖（`pump.rs:70`），无 tmp+rename——与 Linear/anvil 全量写形成对比，是三格式中崩溃安全性最差的。
- `should_write` 恒 true（`pump.rs:50-52`）→ 不依赖 watcher 逐出逻辑，每个保存周期都落盘。

## 6. 区块 NBT 载荷逐字段对照（核心差异表）

写入方：Pumpkin `format/mod.rs:412-614`（`internal_to_bytes`）；Papo `SerializableChunkData`（字段清单 `SerializableChunkData.java.patch:3-10`）。

| NBT 字段 | Pumpkin | Papo/vanilla | 备注 |
|---|---|---|---|
| `DataVersion` | **常量 4903**（`anvil.rs:41`），不随协议版本走 | 实际数据版本 + DFU 升级链 | Pumpkin 读侧完全不解析此字段 |
| `xPos/zPos/yPos` | ✅ | ✅ | |
| `Status` | ✅ 9 态；读侧把 legacy `noise/surface/carvers` 归并为 `terrain`（`format/mod.rs:375-377`） | ✅ 全状态 | |
| `sections[].Y/block_states{palette,data}/biomes` | ✅ | ✅ | Pumpkin 读侧 palette 容忍 IntArray/ByteArray/LongArray/List 四形态甚至模板 palette 条目（`format/mod.rs:73-119`） |
| `BlockLight/SkyLight` | ✅ 原样 nibble | ✅ + **Starlight 私有扩展**：section 级光照状态位 + 写 `isLightOn=false` + `STARLIGHT_VERSION` tag 骗过 vanilla 重点亮（[0001]:33496-33503, 33540-33600） | Papo 世界被 Pumpkin 读时：`isLightOn=false` → 触发重算（行为正确，代价是时间） |
| `Heightmaps` | ⚠️ 只写 3 种（WORLD_SURFACE / MOTION_BLOCKING / MOTION_BLOCKING_NO_LEAVES，`format/mod.rs:315-332`） | ✅ vanilla 全套（含 OCEAN_FLOOR 等） | 缺的高度图客户端可自算，影响有限 |
| `block_ticks/fluid_ticks` | ✅（x/y/z/t/p/i，1.21 字段名） | ✅ | |
| `block_entities` | ✅（原样 NBT 列表） | ✅ | |
| `InhabitedTime` | ✅ | ✅ | |
| `isLightOn` | ✅ | ✅（值被 Starlight 策略化） | |
| `LastUpdate` | ❌ | ✅ | |
| **`structures`**（starts/references/bounding_boxes） | ❌ **完全不写**（`format/mod.rs` 全函数无此 tag；内存 `blending_data: None` :405） | ✅ | **最大缺口**：Pumpkin 存档往返后结构引用丢失——村庄/要塞定位、地图图标、僵尸增援判定等依赖结构数据的机制失准 |
| `blending_data` / `below_zero_retrogen` / `carving_mask` | ❌ | ✅ | 旧世界升级与新旧地形融合相关 |
| 私有 PDC | `PumpkinCustomData`；**读侧回退兼容 `BukkitValues`**（`format/mod.rs:386-390`）——可直接吸收 Paper 世界的区块 PDC | `ChunkBukkitValues` | 唯一的"Pumpkin 主动吃掉 Bukkit 格式"点 |

**读哲学差异**：Pumpkin 是"宽容读"（多种历史形态都接受、坐标错位才报错、不校验版本）；Papo 是"严格读 + DFU"（按 DataVersion 走正规升级，版本高于当前直接 `System.exit(1)` 拒绝降级，`SerializableChunkData.java.patch:28-45`）。

## 7. 实体区块格式（entities/）

| | Pumpkin | Papo |
|---|---|---|
| 容器 | 与区块同配置的 anvil/linear/pump（`level.rs:63-67`） | 恒 MCA（`entities/r.X.Z.mca`，[0001]:26663-26670） |
| NBT | `{ DataVersion: 4903, Position: [x,z] IntArray, Entities: List[Compound] }`（`format/mod.rs:776-794`）；**读侧兼容 legacy `Position-X`/`Position-Z` 独立 int**（:734-747） | vanilla `ChunkEntities` 同构；dfu 通道版本 -1（无历史升级） |
| 与 vanilla 互通 | 结构兼容 ✅（可直接读 vanilla/Paper 的实体区块） | ✅ |
| 上限防护 | 无 | `chunks.entityPerChunkSaveLimit` 每区块实体保存截断（feature 0018，默认 -1 不限） |

## 8. POI 格式（poi/）

| | Pumpkin | Papo |
|---|---|---|
| 覆盖类型 | **仅 `minecraft:nether_portal`**（`poi/mod.rs:21`——村民等 POI 系统未实现，无数据可存） | 全量 vanilla POI（村民作业/床位/蜜蜂…），dfu 版本 1945 |
| 容器 | **手写 MCA 实现**：4 KiB 扇区、zlib（COMPRESSION_ZLIB=2）、硬编码 `DATA_VERSION=3955`（1.21，`poi/mod.rs:26-37`）——与区块格式配置完全独立 | 标准 RegionFileStorage 经 PoiDataController（[0001]:2830） |
| NBT | serde `PascalCase` 映射 vanilla 结构 `{ DataVersion, Sections{ "y": { Valid, Records[{x,y,z,type,free_tickets}] } } }`（`poi/mod.rs:52-77`） | 同 vanilla |
| 互通 | 结构兼容；Papo 世界里的非 portal POI 记录 Pumpkin 原样保留在盘上但不解释 | 双向无损 |

## 9. level.dat / 玩家数据 / 进度统计

| 文件 | Pumpkin | Papo |
|---|---|---|
| level.dat | gzip NBT，`{Data: {...}}` vanilla 容器（serde rename，`world_info/anvil.rs:527`）；**tmp + rename 原子替换**（:441-447），保留 `level.dat_old` 常量（:33）；仅 /save-all 与关停写 | gzip NBT 同构；深拷贝快照 + IO 池 tmp + `safeReplaceFile`（批 80，`LevelStorageSource.java.patch:79-102`）；**autosave 周期也写**（feature 0020） |
| 玩家 `.dat` | gzip NBT（`player_data.rs:99,143`）；**直接 `File::create` 覆盖写，非原子、无备份** | gzip NBT；tmp + `Util.safeReplaceFile(.dat, tmp, .dat_old)` 三文件原子替换 + 备份（`PlayerDataStorage.java.patch:15-33`） |
| 进度 | JSON（`advancement_manager`） | JSON，写盘下放 IO 池（批 79） |
| 统计 | ❌ 未实现 | JSON（stats），同批下放 |

## 10. 互操作矩阵（谁能读谁的盘）

| 盘上数据 ↓ / 读者 → | Pumpkin | Papo（Paper 系） | 原版/Paper 上游 |
|---|---|---|---|
| Pumpkin **anvil** 世界 | ✅ | ⚠️ 大体可读：NBT 是 vanilla 子集 + `PumpkinCustomData` 私有 tag（未知 tag 会被 vanilla 系忽略或保留）；**structures 空** → 结构相关机制失准；LZ4 需 24w04a+ 客户端时代的服务端才认 | ⚠️ 同左（版本足够新时） |
| Pumpkin **linear / pump** 世界 | ✅ | ❌ | ❌ |
| Pumpkin POI/level.dat/玩家 | ✅ | ✅（POI 仅 portal 记录；玩家 NBT 字段是否 vanilla 全集取决于 Pumpkin 写出侧实现完整度） | ⚠️ 同左 |
| Papo/Paper 世界 → Pumpkin 读 | — | ⚠️ **多数区块可读，但**：① 超大块（>255 扇区）读不了（低 8 位截断）；② `.mcc` 外部块不支持；③ `structures`/`blending_data` 字段读入即丢弃（写回时消失）；④ Starlight `isLightOn=false` → 全量重算光照；⑤ `ChunkBukkitValues` 可被读为 PDC ✅；⑥ Custom 压缩块不可解压 | 同左 |

## 11. 格式层风险清单（Pumpkin 侧，按严重度）

1. **sector_count 溢出无断言**（`anvil.rs:243-246` + `:381,463-464`）：>255 扇区块直接写坏位置表且殃及邻居项；实体区块同样暴露。建议加 `debug_assert!`/写 `.mcc` 或强制退化全量写。
2. **不写 `structures`**：跨会话结构引用全丢——这是与 Papo/vanilla 最大的语义缺口（非 merely 字段缺失）。
3. **玩家 `.dat` 非原子覆盖写**（`player_data.rs:140-144`）：崩溃窗口内档案损坏，无 `.dat_old` 兜底。
4. **Pump 格式非原子写 + 恒写盘**（`pump.rs:50-52,70`）：崩溃安全与写放大都最差，应视为实验格式并在文档/配置中标注。
5. `DataVersion` 写死 4903 且读侧不解析：无法表达"这个区块是哪个版本写的"，未来想做迁移时缺少锚点。
6. `Custom`（0x40）压缩可识别不可解压（`anvil.rs:147`）：遇到即整块失败，无降级路径。
7. Heightmaps 只写 3 种（客户端可自算，影响小但非全兼容）。
8. Linear bucket 内 per-chunk timestamp 是规范外自研扩展，第三方 Linear 工具链产物可能不含该字段（读侧 `read_from` 要求 12 字节定长头，`linear.rs:261-268`——遇到官方 LinearRegionFile 文件可能解析错位）。

## 12. 证据索引（本文新增）

**Pumpkin**：`crates/pumpkin-world/src/chunk/format/{anvil.rs, linear.rs, pump.rs, mod.rs}`（三格式全部实现与 NBT 载荷）、`chunk/io/file_manager.rs`、`crates/pumpkin-config/src/chunk.rs`（默认 LZ4-6、write_in_place）、`crates/pumpkin-world/src/poi/mod.rs`（手写 POI MCA）、`crates/pumpkin-world/src/world_info/anvil.rs`（level.dat）、`crates/pumpkin-world/src/data/player_data.rs`、`crates/pumpkin-world/src/level.rs:60-67,151-264`（格式选择与目录）。

**Papo**：[0001]（RegionFile/RegionFileStorage 的 Moonrise 化、Starlight 光照 hack、Entity/PoiDataController）、`patches/sources/net/minecraft/world/level/chunk/storage/{RegionFile,RegionFileStorage,RegionFileVersion,SerializableChunkData,SimpleRegionStorage}.java.patch`、`patches/sources/net/minecraft/world/level/storage/{PlayerDataStorage,LevelStorageSource}.java.patch`、`src/main/java/io/papermc/paper/configuration/{WorldConfiguration,GlobalConfiguration}.java`、features 0004/0009/0017/0018/0019/0020/0022/0066/0129。
