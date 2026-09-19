# 07 · 存档系统对比：Pumpkin（本项目） vs Papo（REF 参考）

> 生成时间：2026-09-19 · 方法：双侧源码精读（Pumpkin 主仓库逐文件精读；Papo 侧经子代理全量扫描补丁树）
> **引用约定**：
> - Pumpkin 侧：`相对路径:行号`，以 `F:\Github\repo\Papokin` 为根，行号为当前 master（c31eb836+）实际行号。
> - Papo 侧：Papo 是 Paper 0.80.0 fork（MC 1.21.11），vanilla 源码不在仓库中，其实现位于
>   ① `paper-server/patches/features/0001-Moonrise-optimisation-patches.patch`（下称 **[0001]**，Moonrise 区块系统全量源码以 new-file diff 内嵌，行号为 patch 文件内行号）
>   ② `paper-server/patches/sources/**/*.patch`（逐文件补丁，行号为补丁文件行号）
>   ③ `paper-server/src/main/java/**`（直提交源码，真实 .java 行号）。

## 0. 对象速写

| | Pumpkin | Papo |
|---|---|---|
| 是什么 | 纯 Rust 从零实现的 MC 服务器 | Java / Paper 0.80.0 深度定制 fork（Moonrise 区块系统） |
| 存档哲学 | **内存即真相**：区块/实体常驻内存（DashMap），保存=把内存快照刷回文件 | **区块系统即真相**：ChunkHolder 状态机管理生命周期，保存=三通道异步 IO 管线 |
| 目录布局 | `region/` `entities/` `poi/` 每维度一套（`level.rs:184-198`），支持 26.2 canonical `dimensions/<ns>/<name>` 与 legacy `DIM-1/DIM1` 回退（`level.rs:157-182`） | 同 vanilla：`region/` `entities/` `poi/`（[0001]:26663-26670） |

## 1. 总体架构对比

```mermaid
graph TB
    subgraph PUM["Pumpkin 保存架构"]
        PW["World::tick_environment<br/>world/mod.rs:1891"] -->|"world_age % autosave_ticks==0<br/>should_save=true + notify"| PS["Schedule 专用线程/每 Level<br/>schedule.rs:1263"]
        PS -->|"save_all_chunk：一次收集全部脏块"| IO["io_write_work tokio 任务 ×1<br/>worker_logic.rs:180"]
        IO --> CS["chunk_saver.save_chunks<br/>按区域分组 join_all"]
        CS --> FM["ChunkFileManager<br/>整文件内存缓存 BTreeMap<br/>无 watcher 才落盘+逐出"]
    end
    subgraph PAPO["Papo 保存架构"]
        PU["ChunkMap.processUnloads 每 tick<br/>[0001]:24189"] -->|"autoSave()：最旧优先<br/>限 max-auto-save-per-tick=24"| PH["NewChunkHolder.save<br/>主线程快照 [0001]:11019"]
        PH -->|"saveExecutor NBT 组包"| MR["MoonriseRegionFileIO<br/>同区块 later-write-wins 合并"]
        MR -->|"compressionExecutor 压缩"| RIO["ioExecutor + AreaDependentQueue<br/>同 region 文件严格串行"]
    end
```

核心结构差异一句话：**Pumpkin 把"地区文件"整体当内存对象**（懒加载进内存 → 修改 → 无 watcher 时全文件重写）；**Papo 把地区文件当随机访问的扇区数据库**（FileChannel 打开、按 chunk 写入扇区、LRU 句柄缓存、同文件操作排队串行）。

## 2. 保存触发与节奏

### 2.1 触发点对照

| 触发 | Pumpkin | Papo |
|---|---|---|
| 周期 autosave | `world_age % autosave_ticks == 0` → `should_save` 标志 + 通知（`world/mod.rs:1891-1896`），默认 6000 tick（`pumpkin-config/src/world.rs:21,41`） | bukkit.yml `ticks-per.autosave` 默认 6000（`CraftServer.java:473`），区块级 `chunks.auto-save-interval` 回落到它；消费在 `ChunkHolderManager.autoSave()`（[0001]:6976-7005） |
| autosave 粒度 | **一次全量扫**：`save_all_chunk(false)` 遍历全部 holder 收集所有脏 Level 区块（`schedule.rs:917-960`）→ 一次突发 IO | **增量摊销**：`autoSaveQueue`（按 lastAutoSave 最旧优先的有序集）每 tick 最多取 `max-auto-save-chunks-per-tick`（默认 24）个 |
| 区块卸载 | 每 100 tick `should_unload` → `clean_memory`；卸载队列每 1 秒批量处理（`schedule.rs:1302-1306`）；脏块保存（`schedule.rs:887-890`） | 三段式 `unloadStage1/2/3`（[0001]:10211-10315），stage2 保存 chunk/实体/POI；卸载延迟可配 `chunks.delay-chunk-unloads-by`（默认 10s，`WorldConfiguration.java:530`） |
| /save-all | `Server::save_all`（`server/mod.rs:585-611`）：**level.dat/玩家/进度同步**，区块仅置 `should_save`+notify **异步触发、不等待完成**（`world/mod.rs:7101-7106`）；`flush` 子参数当前不改变行为（`saveall.rs:67`） | `saveAllChunks(flush)`：逐块保存 + 每 100 块 partialFlush + 结尾 `MoonriseRegionFileIO.flush` + `flushRegionStorages`，且 `PapoOrderedFileWrites.awaitAll(60s)` **等全部落盘才返回**（[0001]:7007-7096；`MinecraftServer.java.patch:552-554`） |
| save-off/on | `save_enabled` 原子标志，手动 save-all 仍可保存（`level.rs:114-116`） | vanilla 语义保留 |
| 关停 | 见 §7 | 见 §7 |
| 插件钩子 | `WorldSaveEvent`（`world/mod.rs:7107-7112`） | 无直接存档事件（实体侧有 EntitiesUnloadEvent/EntitiesLoadEvent，`PersistentEntitySectionManager.java.patch:89-160`） |

**关键语义差**：Papo 的 `/save-all` 是完成性契约（返回=数据在盘，备份工具可依赖）；Pumpkin 的 `/save-all` 对区块是 fire-and-forget，命令反馈发出时区块可能仍在写。同时 Pumpkin 的 autosave 是周期性 IO 风暴（一次刷全部脏块），Papo 是每 tick 平滑限流。

## 3. 区块写盘管线与线程模型

### 3.1 Pumpkin 管线（`chunk/io/file_manager.rs`）

1. Schedule 专用线程决策后，脏块经 mpsc 发给每 Level **1 个** `io_write_work` tokio 任务（`schedule.rs:129`，读任务 4 个）；proto 区块先 `spawn_blocking` 升级为 Level 区块（`worker_logic.rs:196-212`）。
2. `save_chunks` 按区域文件分组 `join_all` 并行（`file_manager.rs:327-410`）；**同一文件天然由 `RwLock<S>` 串行**（每路径一个懒加载 serializer，`file_manager.rs:42-46`）。
3. 每个区块：`is_dirty()` → `mark_dirty(false)` **原子清脏后再序列化**（写期间的变更会重新标脏，`file_manager.rs:360-373`）→ `update_chunk` 把 NBT 压缩进内存中的整文件结构。
4. **落盘条件**：该文件已无 watcher（`file_manager.rs:381-404`）才调 `serializer.write()` 整文件写盘并逐出缓存；有玩家注视的 region 文件**只驻留内存不落盘**（靠 autosave 周期到来时也无济于事——仍不写，直到 unwatch）。flush 屏障 `block_and_await_ongoing_tasks` 用"取写锁即放"实现（`file_manager.rs:420-439`）。

### 3.2 Papo 管线（Moonrise，[0001]）

1. 主线程：`SerializableChunkData.copyOf` 快照（[0001]:11083）→ `tryMarkSaved()` 立即清脏（:11086）→ 快照交给 `saveExecutor`（worker 池）组包 NBT。
2. `MoonriseRegionFileIO.scheduleSave` 以 chunkKey 合并任务——同区块多次写 **later-write-wins**（[0001]:1627-1650）。
3. `compressionExecutor`（worker 池）做 NBT 序列化+压缩进 RegionFile 的 ChunkBuffer（[0001]:2445-2480）。
4. `ioExecutor`（IO 池）真正写文件；`AreaDependentQueue(ioExecutor, shift=5)` 保证**同 region 文件严格串行、不同文件并行**（[0001]:2635-2643）。
5. 线程数：worker 池 `cores/2 clamp[2,12]`、region IO 池 `cores/8 clamp[1,4]`（Papo 定制，`io/papermc/paper/util/PapoParallelism.java:41-59`），可用 `chunk-system.{worker,io}-threads` 或 JVM 属性覆盖。

### 3.3 对照要点

| 维度 | Pumpkin | Papo |
|---|---|---|
| 序列化线程 | tokio `spawn_blocking`（未指定池上限） | 专用 saveExecutor + compressionExecutor 两级 |
| 写盘线程 | 每 Level 1 个写任务（tokio） | 全局 IO 池 + 同文件有序队列 |
| 同文件并发 | 每-文件 RwLock（粗粒度：压缩+写盘都在锁内） | AreaDependentQueue（细粒度：仅文件写串行） |
| 写合并 | 无（save_chunks 批次内同文件块合并一次写） | 跨批次按区块键合并，后写覆盖前写 |
| 清脏时机 | 序列化前（`file_manager.rs:365`） | 快照后立即（[0001]:11086）——**两侧同为"先清脏"，失败恢复策略不同（§6）** |

## 4. 地区文件格式

| 维度 | Pumpkin | Papo |
|---|---|---|
| 格式清单 | **三格式**：anvil(.mca) / Linear V2 / 自研 Pump（`level.rs:60-67`，由 `ChunkConfig` 选择，`level.rs:251-264`） | **仅 anvil(.mca)**；全仓库无 LinearRegionFile（探查确认零命中） |
| 压缩算法 | anvil 每-块版本字节 GZip/ZLib/LZ4/Custom（`chunk/format/anvil.rs:45-52`），配置 `compression`（`pumpkin-config/src/chunk.rs:33`） | GZIP/ZLIB(默认)/LZ4/NONE 可配（`RegionFileVersion.java.patch:7-16`）+ **压缩级别**可配（默认 6=与 vanilla 逐字节一致，feature 0066）+ Deflater/Inflater 池化（feature 0129） |
| 写入方式 | **整文件重写**：`WriteAction::{Pass,All,Parts}`——无脏块跳过 / tmp 文件全量重建 / `write_in_place` 按索引局部写（`anvil.rs` ChunkSerializer impl；`write_all` tmp+重建 header，`pumpkin-config/src/chunk.rs:35`） | **扇区级随机写**：FileChannel + ChunkBuffer 定位扇区写入；Spigot 255 扇区扩展 + 超 1MB 外部 `.mcc` 文件（`RegionFile.java.patch:13-23,49-60`；features 0004/0009） |
| 文件缓存 | BTreeMap 全量持有**整个文件内容在内存**，仅靠 watcher 计数逐出（**无容量上限**，`file_manager.rs:43-44`） | RegionFile 句柄 LRU 缓存上限 256（`misc.region-file-cache-size`，[0001]:32932-32950）+ 不存在文件负缓存 4096（:32909-32940） |
| 损坏容错 | 读侧宽容：palette 接受 IntArray/ByteArray/LongArray/List 多形态、legacy Status 字符串映射、坐标错位报错（`chunk/format/mod.rs:73-119,370-384,194-199`） | region 头损坏时**重算 header** 而非搬文件（feature 0019）；序列化抛异常则不写盘保住旧版本（feature 0017） |
| fsync | 无显式 fsync 控制 | 构造时 dsync 可选 + `chunks.flush-regions-on-save` 每写即 flush 元数据（feature 0022） |

**内存代价是 Pumpkin 模型的隐性风险**：每个被缓存的 region 文件（最多数 MB）整体驻留，且无 LRU 上限——大范围探索的服务器中，`file_locks` 只增不减（仅 watcher 逐出），而"有玩家注视→不落盘"意味着热门区域长期不产生磁盘备份点。

## 5. 区块 NBT 内容与数据版本

| 维度 | Pumpkin | Papo |
|---|---|---|
| 写出的字段 | DataVersion(**常量 4903**, `anvil.rs:41`)、xPos/zPos/yPos、Status、Heightmaps、sections（block_states/biomes/BlockLight/SkyLight）、block_ticks/fluid_ticks、block_entities、isLightOn、InhabitedTime、PumpkinCustomData（`chunk/format/mod.rs:455-613`） | 完整 vanilla `SerializableChunkData`：另有 **structures**（结构起始/引用/包围盒）、blending_data、below_zero_retrogen、carving_mask、LastUpdate、ChunkBukkitValues(PDC)（`SerializableChunkData.java.patch:3-10`）+ Starlight 光照版本号 hack（写假 isLightOn=false + STARLIGHT_VERSION，[0001]:33496-33503） |
| **缺失项** | **不保存 structures**（format/mod.rs 无该 tag——结构引用丢失，重载后结构会重新判定）与 blending/carving；实体区块自定义格式 `DataVersion/Position/Entities`（`format/mod.rs:776-794`） | 无缺失 |
| DataVersion 读取 | **不读取** `DataVersion`，无任何版本分支 | 完整 DFU 升级链：`upgradeChunkTag`（legacyFixer→injectDatafixingContext→`dataFixType.updateToCurrentVersion`，`SimpleRegionStorage.java.patch:24-52`）；POI（版本 1945）与实体区块同样走 dfu（[0001]:1000-1040）；**降版本保护：DataVersion > 当前 → System.exit(1)**（`SerializableChunkData.java.patch:28-45`） |
| 第三方世界兼容 | 读侧兼容 Bukkit：`PumpkinCustomData` 回退读 `BukkitValues`（`format/mod.rs:386-390`）；legacy Status 名合并为 Terrain（:375-377） | 双向 vanilla 兼容（含 CB/Paper 扩展字段） |

**结论**：Papo 背靠 20 年 DataFixerUpper 资产，可安全读写任意历史版本存档；Pumpkin 写死当前版本号、不读不升级，靠宽容解析"尽力读"，跨版本升级世界（如 1.20 → 1.21+）不在支持范围内，且写回会**丢失结构数据**。

## 6. 失败语义

| 场景 | Pumpkin | Papo |
|---|---|---|
| 写盘失败 | `save_chunks` 返回 Err → `error!` 日志，**无重试**（`level.rs:858-863`）；脏标志已在序列化前清除（`file_manager.rs:365`）→ 该批改动需等下次变更重新标脏才会再存 | 写失败时任务**保留在 chunkTasks map 防数据丢失**，新数据到来会重新走压缩/写盘（[0001]:2515-2522, 2478-2505）；序列化异常不写盘保旧版（feature 0017） |
| 快照-清脏窗口 | 与 Papo 相同的"先清脏"模式，但失败后无兜底 | 同左，但有 map 兜底 |
| 玩家 .dat 写入 | **直接 `File::create` 覆盖写** gzip NBT，非原子（`pumpkin-world/src/data/player_data.rs:140-144`）——写到一半崩溃=档案损坏 | tmp 文件 + `Util.safeReplaceFile(.dat, tmp, .dat_old)` 三文件原子替换 + 备份（`PlayerDataStorage.java.patch:15-33`） |
| level.dat 写入 | tmp + rename 原子替换（`world_info/anvil.rs:441-447`），有 `level.dat_old` 常量（:33） | 深拷贝快照 → IO 池 tmp + safeReplace（批 80，`LevelStorageSource.java.patch:79-102`） |

## 7. 关停与崩溃

### Pumpkin 关停序列（`server/mod.rs:747-767` → `level.rs:356-425`）

1. `Server::shutdown`：等 server 级任务 → 各 `World::shutdown`：实体按区块快照保存 + block entities + portal POI → `Level::shutdown`。
2. `Level::shutdown`：cancel token → 通知 chunk 线程 → **Thread-Joiner 3 秒超时 join**（`level.rs:389`，超时仅告警继续）→ `block_and_await_ongoing_tasks` → 把全部内存实体区块写出。
3. Schedule 线程收到 `shut_down_chunk_system` 后 `save_all_chunk(true)`——**含 proto（半生成）区块**（`schedule.rs:1268-1274`）。
4. 最后写 level.dat。

### Papo 关停序列（`MinecraftServer.java.patch:599-620`；[0001]:6923-6970, 23232-23290）

1. `stopServer` → "Saving players"（`PapoOrderedFileWrites.awaitAll(60s)`）→ "Saving worlds"（`saveAllChunks(flush, close=true)`）。
2. `ChunkHolderManager.close`：等 chunk 系统 halt（**60s**）→ `saveAllChunks(shutdown=true)`（同步执行排队中的卸载任务、**强制 transient 实体区块落盘**，[0001]:11052-11057）→ `MoonriseRegionFileIO.flush` → 等 IO halt（60s）→ 关闭全部 region 缓存。
3. `MoonriseCommon.haltExecutors()` 两池 graceful 排水（各 60s，超时强制 halt，`MoonriseCommon.java:96-110`）。
4. 另有**紧急保存**路径 `issueEmergencySave`（[0001]:23179-23191）与崩溃时 `saveAllChunks(emergency=true)`。

**差异本质**：Pumpkin 关停预算 3 秒（超时放弃等待，靠后续 `block_and_await` 与实体直写兜底）；Papo 全链路 60 秒量级的多阶段排水 + 紧急通道。Pumpkin 大世界慢盘下更可能截断区块保存。

## 8. 实体持久化

| 维度 | Pumpkin | Papo |
|---|---|---|
| 内存形态 | `ChunkEntityData{ x, z, data: Mutex<Vec<NbtCompound>>, live, dirty }`（`level.rs:328-334`） | `ChunkEntitySlices`（分节实体容器，含 transient 标志，[0001]:3221-3228） |
| 常规保存 | 无 watcher 的实体区块移除后异步整写（`level.rs:485-517`）；live 标志跟踪"该区块实体曾被加载进世界" | 卸载/autosave/save-all 时经 ENTITY_DATA 通道写 `entities/r.X.Z.mca`（[0001]:11136-11191） |
| **写盘语义** | **快照整写**：`save_entities_by_chunk` 把内存中全部实体按 chunk_pos 重新分组，live 区块整体重建（空了也重写）；从未 live 且空的区块保留磁盘记录（`world/mod.rs:598-640`；commit 04d0276 修复追加式重复） | **读-改-写合并**：transient 区块保存时先读回磁盘旧数据 `copyEntities(onDisk, save)` 合并再写，防止覆盖盘上已有实体（[0001]:11149-11168）；transient 非卸载时不保存（防双加） |
| 序列化时机 | tick 之外（spawn_task / 关停路径），write_nbt 在异步上下文 | **红线：必须在 `entityChunk.unload()`（卸载事件+setRemoved）之前同步序列化**，否则落盘字节≠vanilla——Papo 明确否决了 IO 下放（`note/optimizations.md:2135`） |
| 数量防护 | 无 | `chunks.entityPerChunkSaveLimit` 每区块实体保存上限（feature 0018）+ 每区块掉落物上限（feature 0209） |

两种语义是**同一问题的两种正确解**：Papo 的 transient 合并保字节等价（vanilla 磁盘上可能有本进程未见过的实体）；Pumpkin 的 live 快照整写依赖"live 区块内内存实体集合=完整真相"这一更强的不变式（代价是磁盘上曾有、但本进程未加载的记录在 live 化后被覆盖——由"从未 live 保留磁盘"分支规避了大部分场景）。

## 9. 玩家数据 / level.dat / POI / 统计进度

| 项 | Pumpkin | Papo |
|---|---|---|
| 玩家保存时机 | 退出同步（`player_server.rs:47-60`）；周期快照（间隔 `save_player_cron_interval` 配置，`server/mod.rs:241-244`；**快照在 tick 线程做、写盘 rayon::spawn**，`player_server.rs:66-98`）；关停同步 `save_all_players` | 退出同步；周期增量（`player-auto-save.rate` + 每 tick 上限 10-20，feature 0020）；全部经 `PapoOrderedFileWrites`（per-UUID 有序链，gzip+写盘在 IO 池，主线程仅深拷贝快照——批 79 基准 50 玩家突发 89ms→2ms） |
| 玩家读路径 | 直接读 gzip | 读前 `awaitPending(file)` 保证快速重连的读后写可见性（`PlayerDataStorage.java.patch:44-50`）；登录 RTT 窗口**预取** .dat/stats/advancements（批 82/0249，主线程 15.45ms→0.04ms） |
| level.dat | gzip NBT tmp+rename；**仅 /save-all 与关停写**，autosave 不写 | autosave 周期（doFull）也写（feature 0020）；写入下放 IO 池（批 80）；备份前 awaitPending |
| POI | **仅下界传送门 POI**（`poi/mod.rs:21` 只有 `POI_TYPE_NETHER_PORTAL`），MCA+zlib 手写实现（DATA_VERSION 3955，`poi/mod.rs:28,37`），save/shutdown 时同步 `save_all` | **完整 vanilla POI**（村民作业点等）；vanilla SectionStorage 自管 flush 被禁用，统一走 PoiDataController 区块系统通道（[0001]:28894+；33224） |
| 统计/进度 | 进度 JSON（`advancement_manager`）；统计系统未实现 | stats/advancements JSON：编码在调用线程快照、写盘入队 IO 池（批 79），各有 disable 开关 |

## 10. 配置面对照

| 配置 | Pumpkin（pumpkin.toml） | Papo（paper/bukkit/spigot yml） |
|---|---|---|
| autosave 间隔 | `[world] autosave_ticks`（默认 6000，`pumpkin-config/src/world.rs:21`） | bukkit `ticks-per.autosave: 6000` + paper `chunks.auto-save-interval`（回落前者） |
| 每 tick 保存量 | **无**（一次全量） | `chunks.max-auto-save-chunks-per-tick: 24` |
| 区块格式 | `[chunk] anvil{compression,write_in_place} / linear / pump` | 无格式选择（仅 anvil） |
| 压缩 | anvil.compression: GZip/ZLib/LZ4/Custom | `unsupported-settings.compression-format`（GZIP/ZLIB/LZ4/NONE）+ `compression-level`（默认 6） |
| IO 线程 | 固定 4 读 1 写/Level（`schedule.rs:120-133`） | `chunk-system.io-threads`（auto=cores/8[1,4]）/`worker-threads`（auto=cores/2[2,12]） |
| region 缓存 | 无上限（watcher 逐出） | `misc.region-file-cache-size: 256` |
| 每写即 flush | 无 | `chunks.flush-regions-on-save: false` |
| 卸载延迟 | 硬编码 1s 批处理（`schedule.rs:1302`） | `chunks.delay-chunk-unloads-by: 10s` |
| 玩家周期保存 | `player_data.save_player_cron_interval` / `save_player_data` | `player-auto-save.rate` / `.max-per-tick` |
| 关停/保存开关 | `/save-off` `/save-on`；`save_player_data`、`save_advancements` 布尔 | spigot `players.disable-saving` / stats/advancement disable 系列 |

## 11. 总结

### 设计哲学

- **Pumpkin**：用 Rust 所有权模型把存档做"厚内存薄管线"——区块/实体是常驻并发结构（DashMap+原子字段），保存只是把内存快照压缩回文件；格式层是干净的可插拔 trait（`ChunkSerializer`，anvil/linear/pump 三实现），实体语义靠最近修复的 live 快照整写保证一致性。工程简洁，但**完成性契约、失败兜底、版本迁移三块都还是薄板**。
- **Papo**：在 Paper/Moonrise 的成熟骨架上做"主线程阻塞文件 IO 清算"——三通道（区块/POI/实体）region-IO 池化、同文件串行、later-write-wins 合并、DFU 全版本升级，再叠 Papo 自己的 `PapoOrderedFileWrites` 有序写链与 60s 排水关停。处处是十年运维教训的防御（.dat_old 备份、header 重算、紧急保存、写失败保数据）。

### Pumpkin 相对优势

1. **格式广度**：Linear V2 与自研 Pump 格式、`write_in_place` 局部写选项——Papo 无格式选择。
2. **实体快照语义简单可推理**：live=内存真相，无读-改-写合并的时序坑（代价见 §8）。
3. **插件可见性**：存档有 `WorldSaveEvent` 钩子；区块级 PDC（`PumpkinCustomData`）双向兼容 BukkitValues。
4. **内存模型天然异步**：序列化在 spawn_blocking/rayon，主 tick 从不等文件 IO（Papo 实体卸载序列化仍在主线程——它自己的红线决定）。

### Pumpkin 相对短板（如需向 Papo 看齐的改进清单）

1. `/save-all` 不等待区块落盘即返回（备份工具不可依赖）；建议增加 flush 语义：await `block_and_await_ongoing_tasks` + 排空 io_write 通道。
2. autosave 无每 tick 限流，大脏块集=IO 风暴；可借鉴最旧优先+限速队列。
3. 写失败仅日志、脏标志已清——丢改动窗口；可保留失败任务重试。
4. 玩家 `.dat` 非原子写（直接 File::create 覆盖）；应 tmp+rename+`.dat_old`。
5. region 文件整文件驻留内存且缓存无上限；应加 LRU 上限（Papo 256）。
6. **不写 structures tag、不读 DataVersion**：跨版本世界与结构持久化缺口（最大兼容性差距）。
7. POI 仅传送门（村民系统未落地所致）。
8. 关停 3s join 预算偏紧，无紧急保存路径。

### Papo 值得借鉴但 Pumpkin 暂不需要照搬的

- DFU 升级链是 Java 生态独有资产，Rust 侧等价物需自建 schema 迁移框架，成本高——短期更现实的是"宽容读 + 写当前版本 + 拒绝降版本"三件套（Pumpkin 已有前两者）。
- Deflater 池化/压缩级别调优属 Java GC 语境优化，Rust 无此压力。

## 12. 证据索引（关键文件）

**Pumpkin**：`crates/pumpkin-world/src/level.rs`（Level/shutdown/entity chunks）、`chunk/io/file_manager.rs`（ChunkFileManager 全部保存逻辑）、`chunk/io/mod.rs`（FileIO/ChunkSerializer trait）、`chunk/format/mod.rs`（区块/实体 NBT 编解码）、`chunk/format/anvil.rs`（anvil 读写/压缩/WriteAction）、`chunk_system/schedule.rs`（should_save/should_unload 消费、save_all_chunk）、`chunk_system/worker_logic.rs`（io_write_work）、`crates/pumpkin/src/world/mod.rs:575,7069`（World::save/shutdown、autosave 触发）、`crates/pumpkin/src/server/mod.rs:585,747`（save_all/shutdown）、`crates/pumpkin/src/data/player_server.rs`、`crates/pumpkin-world/src/data/player_data.rs`、`crates/pumpkin-world/src/poi/mod.rs`、`crates/pumpkin-world/src/world_info/anvil.rs`、`crates/pumpkin/src/command/commands/saveall.rs|saveoff.rs|saveon.rs`、`crates/pumpkin-config/src/{world,chunk,player_data}.rs`。

**Papo**：`REF/Papo-Java-0.80.0-src/paper-server/patches/features/0001-Moonrise-optimisation-patches.patch`（Moonrise 全部）、`patches/sources/net/minecraft/world/level/chunk/storage/*.patch`（RegionFile/RegionFileStorage/RegionFileVersion/SerializableChunkData/SimpleRegionStorage）、`patches/sources/net/minecraft/world/level/storage/{PlayerDataStorage,LevelStorageSource}.java.patch`、`patches/sources/net/minecraft/server/MinecraftServer.java.patch`、`src/main/java/io/papermc/paper/util/{PapoOrderedFileWrites,PapoParallelism}.java`、`src/main/java/ca/spottedleaf/moonrise/common/util/MoonriseCommon.java`、`src/main/java/io/papermc/paper/configuration/{WorldConfiguration,GlobalConfiguration}.java`、features 0004/0009/0017/0018/0019/0020/0022/0066/0129/0209/0249、`note/optimizations.md`（批次决策日志，含实体下放否决记录 :2135）。
