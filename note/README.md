# Papokin 项目分析笔记

> 基线：2026-09-19 由 codebase-analyzer 技能生成（采样深度分析模式），此后随代码演进持续修订。
> 范围：`F:\Github\repo\Papokin` 全仓库，**排除 `REF/` 参考资料目录**（用户指定）
> 规模：20 个 workspace 成员（18 crate + 2 tools）· 2789 个 Rust 文件 · 约 150 万行 · 测试基线 1124 通过（2026-09-29）
>
> **快照说明**：01–06 为架构分析文档，已随 Bedrock 移除与 Papokin 改名同步修订；其中 `文件:行号` 形式的证据标注可能随代码演进漂移，论断以就近代码为准。历史记录类文档（07–11、13–15）按成文时点保留，记录中的旧状态是其历史事实的一部分。

## 项目一句话

**Papokin**：纯 Rust 编写的 Minecraft Java 版服务器（1.21.11 协议），以 WASM 组件模型（wasmtime + WIT，`papokin:plugin@0.1.0`）为插件体系，20TPS 专用线程 + rayon 并行模拟 + tokio 异步网络三执行域协同。项目为 [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) 的分支，2026-09-22 整体改名，基岩版支持已于 2026-09-21 移除（见 [11 §十](11-插件API强化实现记录-Papo机制级覆盖.md)）。

## 笔记目录

| 文档 | 内容 | 图表 |
|------|------|------|
| [01-项目概览.md](01-项目概览.md) | 快照、技术栈、规模分布、分层架构总图、核心发现 | 系统分层图（Mermaid） |
| [02-目录结构.md](02-目录结构.md) | 根目录、18 crate 逐目录职责注解、tools、CI、运行期目录 | 数据源→生成→产物关系图 |
| [03-依赖关系.md](03-依赖关系.md) | workspace 内部依赖全图、分层规则、外部依赖分类、feature 体系、插件生态依赖小宇宙 | 内部依赖图、分层图、插件依赖图 |
| [04-模块分析.md](04-模块分析.md) | 服务器核心/Ticker/网络/世界引擎/实体/AI/插件/命令/方块/配置/支撑库 逐模块剖析 | Server 协作图、tick 流程图、包管线图、状态机、区块阶段流水线、实体层次图、插件分层图 |
| [05-数据流.md](05-数据流.md) | 总体数据流、入站/出站包变换表、区块加载/保存时序、认证时序、命令流、插件事件时序 | 总数据流图、出站流程、加载时序图、认证时序图、事件时序图 |
| [06-控制流.md](06-控制流.md) | 启动时序、线程模型全图、accept/tick 双循环、连接生命周期、关停序列、panic/崩溃路径 | 启动时序图、线程模型图、连接流程图、关停时序图、panic 流程图 |
| [07-存档系统对比-Pumpkin-vs-Papo.md](07-存档系统对比-Pumpkin-vs-Papo.md) | 与 REF/Papo（Paper fork）的存档系统逐维对比：触发节奏、写盘管线、格式、实体语义、失败/关停、配置面、互操作性 | 双侧保存架构对比图 |
| [08-存档格式深挖-Pumpkin-vs-Papo.md](08-存档格式深挖-Pumpkin-vs-Papo.md) | 磁盘字节层对比：MCA 逐字节差异、压缩策略、Linear V2/Pump 格式解剖、区块 NBT 逐字段表、实体/POI/level.dat/玩家格式、互操作矩阵、格式风险清单 | 磁盘布局树、Linear V2 结构图 |
| [09-存档系统重写实现记录.md](09-存档系统重写实现记录.md) | 实现记录：MC 版本切至 1.21.11；Papo 兼容 RegionFile（255 扩展/.mcc/oversized/头自愈/原子写/扇区溢出防护）、区块 NBT 全字段保留、玩家/level.dat/POI 原子写、保存管线防丢；230 测试全绿 | — |
| [10-插件系统对比-Pumpkin-vs-Papo.md](10-插件系统对比-Pumpkin-vs-Papo.md) | 与 REF/Papo（Paper fork）的插件系统逐维对比：WASM 能力沙箱 vs JVM 信任模型、加载/生命周期/类加载、事件分发、权限双语义、命令/调度/IPC、沙箱与供应链、配置面、互鉴清单（成文时点快照；清单 6 项当日已落地，见 11） | 双侧架构对比图、事件分发对比图 |
| [11-插件API强化实现记录-Papo机制级覆盖.md](11-插件API强化实现记录-Papo机制级覆盖.md) | 实现记录：EventPriority+ignoreCancelled 分发、异步任务、依赖分级、ServicesManager、插件消息通道、config 深合并、命令 fallback 前缀、permissions.toml、Startup 引导阶段、事件 fire 点补缺 47 处（含不可接线清单）；API 版本 2→3；e2e wasm 插件 7 标记全绿；§十 基岩版移除（版本 0.3.0）；§七为后续两轮代码审计的稳定性修复清单 | — |
| [12-插件API文档.md](12-插件API文档.md) | 插件开发者参考文档：架构总览、快速上手（wasm32-wasip2 构建/部署/热重载）、生命周期与依赖、事件系统（367 类型/优先级/取消语义）、调度器（含 EntityScheduler 与即时取消语义）、命令、双层权限（含运行时附件命名空间约束）、配置（原子写）、服务/IPC/插件消息、数据存储（数据文件夹+PersistentDataHolder）、Server/World/Entity/Player 方法面、AI 目标（内建 + AiGoalManager 自定义注册）、世界生成（GeneratorManager）、沙箱权限与日志、API 面统计与版本策略 | — |
| [13-插件API覆盖复核-当前代码vs-Papo.md](13-插件API覆盖复核-当前代码vs-Papo.md) | 覆盖复核（锚定 b6af9b3c7）：11 的 12 项机制逐项验证属实；拉宽到 Papo 全 API 面的子系统覆盖矩阵（org.bukkit 1268 文件 + Paper 扩展）；896 WIT 函数/273 事件/288 fire 点实测；剩余缺口排序（Registry/Tag 体系最大）；勘误：world.spawn-entity/get-entities 存在，EntityScheduler 可无头 e2e | — |
| [14-全仓汉化实现记录.md](14-全仓汉化实现记录.md) | 实现记录：全仓 Rust 源码注释+运行时文本汉化（2026-09-22）；范围/规则/管线、应用统计、门禁全绿、引出的 clippy/fmt/断言同步坑与修复清单；现行规范见仓库根 `tmp_i18n_guide.md` | — |
| [15-安全与稳定性审计-入站包输入信任.md](15-安全与稳定性审计-入站包输入信任.md) | 持续审计（2026-09-26 起，十轮）：网络入界面（61 个 play 处理器+登录前网络面+RCON/代理/认证）、存档完整性（NBT 写侧/方块实体/实体快照/保存管线）、事件取消语义（413 fire 点全核）、写放大与关停、库存/容器并发（条带锁+原子槽位原语）、tick panic 隔离、实体高负载上界；§十五 反作弊职责边界调整（玩法稽查全部移交插件，两轮裁决）；§十六 部署后热修（纹理域名白名单归一化、挖掘阈值 0.8、旧版客户端挖掘工具对应错乱五连修：block_id_remap 生成器+UpdateTags 重映射+首条匹配语义、磁盘深度清理 24GB 与 tools/clean_stale_artifacts.py（二轮再清 16.44GB：修脚本漏扫根 target/指纹键名不匹配双 bug、新增被取代同单元产物规则、`~/.cargo` 修剪事故补记）、UpdateTags 全类别跨版本 id 审计（game_event 省略/fluid 换序，**fluid 换序后经勘误移除**——浸水岩浆红屏根因）与挖掘速度水下/漂浮修正）；最终基线 1124+ 测试通过 | — |

## 核心发现（十件事）

1. **三执行域**：tokio（网络/IO/插件驱动）+ rayon 全局池（并行模拟）+ 每世界 `ChunkGen` 专用池；`main.rs:44` 的"rayon 不得阻塞 tokio"是全库并发纪律。
2. **单一 tick 真源**：`Server-Ticker` 专用线程 50ms/tick，世界/玩家/实体/方块计划 tick/刷怪全部 rayon 分批并行；网络包在 `Player::tick` 开头消费（≤64/tick）。
3. **红石双路径**：tick 开头 `flush_synced_block_events`（同步方块事件）+ `tick_chunks` 第一步计划 block tick（`world/mod.rs:1513,1963`）。
4. **区块生成 DAG**：`StagedChunkEnum` 11 阶段（Empty→Biomes→…→Lighting→Spawn→Full），`Schedule` 专用线程 + 4 读 1 写 IO 任务 + 生成池；保存支持 anvil/linear/pump 三格式，实体区块按快照整写。
5. **插件即 WASM 组件**：WIT world `papokin:plugin@0.1.0`（API v6；30+ host import/10+ guest export），宿主与插件两侧类型都从同一契约生成，CI 强制校验不漂移。
6. **事件可否决**：`PluginManager::fire` 先串行 blocking handler（可修改/取消事件）再非 blocking；命令、聊天、包收发等关键路径均有 veto 点（取消语义经 15 §十二全量核修）。
7. **Brigadier 克隆命令树**：`ArcSwap<CommandDispatcher>` 支持插件装卸时**无锁换树并全体重发**命令数据包；权限谓词在解析期过滤（无权限命令不可见）。
8. **数据即代码**：`assets/*.json` → `tools/papokin-codegen` → 73MB 静态 Rust 数据，零运行时解析成本；`*_id_remap` feature 与 `sync_id_remap` 表服务存档版本重映射（1.21.11 存档重写）。
9. **不信任用户输入**：网络入界面做完整性与资源校验（登录序列一次性门控、入站水位、协议状态机）；玩法层面的稽查（移动速度/交互距离执法）不在此列——反作弊职责已整体移交插件（见 [15 §十五](15-安全与稳定性审计-入站包输入信任.md)）；存档写侧带体积预算与原子替换。
10. **生产级健壮性**：clippy deny unwrap/expect/panic；刻内 panic 隔离（坏包只踢当事人）；panic→崩溃报告→优雅关停；30s 登录超时、keep-alive、包限速、WASM 沙箱（内存/socket/目录/签名）层层设防。

## 证据约定

所有论断均以 `文件路径:行号` 形式内联标注（如 `server/ticker.rs:20` 指 `crates/papokin/src/server/ticker.rs` 第 20 行）。行号随代码演进可能漂移，以就近代码为准。

## 分析方法说明

- 大目录（`entity/` 290 文件、`net/java/play/` 61 处理器、`mob/` 50 文件）采用"全量目录扫描 + 代表文件精读"的采样策略。
- 深度追踪由 3 个并行子代理完成（服务器核心/插件系统/命令配置认证），网络、世界、实体、支撑库由主分析直接完成。
- `REF/` 目录按要求完全未读取。
