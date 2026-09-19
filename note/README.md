# Papokin (Pumpkin) 项目分析笔记

> 生成时间：2026-09-19 · 工具：codebase-analyzer 技能（采样深度分析模式）
> 范围：`F:\Github\repo\Papokin` 全仓库，**排除 `REF/` 参考资料目录**（用户指定）
> 规模：20 个 workspace 成员 · 2718 个 Rust 文件 · 约 150 万行 · 978 个测试

## 项目一句话

**Pumpkin**：纯 Rust 编写的 Minecraft 服务器，同时支持 Java 版（TCP）与 Bedrock 基岩版（WebRTC/NetherNet）最新协议，以 WASM 组件模型（wasmtime + WIT）为插件体系，20TPS 专用线程 + rayon 并行模拟 + tokio 异步网络三执行域协同。

## 笔记目录

| 文档 | 内容 | 图表 |
|------|------|------|
| [01-项目概览.md](01-项目概览.md) | 快照、技术栈、规模分布、分层架构总图、核心发现 | 系统分层图（Mermaid） |
| [02-目录结构.md](02-目录结构.md) | 根目录、18 crate 逐目录职责注解、tools、CI、运行期目录 | 数据源→生成→产物关系图 |
| [03-依赖关系.md](03-依赖关系.md) | workspace 内部依赖全图、分层规则、外部依赖分类、feature 重映射体系、插件生态依赖小宇宙 | 内部依赖图、分层图、插件依赖图 |
| [04-模块分析.md](04-模块分析.md) | 服务器核心/Ticker/网络/世界引擎/实体/AI/插件/命令/方块/配置/支撑库 逐模块剖析 | Server 协作图、tick 流程图、包管线图、状态机、区块阶段流水线、实体层次图、插件分层图 |
| [05-数据流.md](05-数据流.md) | 总体数据流、入站/出站包变换表、区块加载/保存时序、认证时序、命令流、插件事件时序 | 总数据流图、出站流程、加载时序图、认证时序图、事件时序图 |
| [06-控制流.md](06-控制流.md) | 启动时序、线程模型全图、accept/tick 双循环、连接生命周期、关停序列、panic/崩溃路径 | 启动时序图、线程模型图、连接流程图、关停时序图、panic 流程图 |
| [07-存档系统对比-Pumpkin-vs-Papo.md](07-存档系统对比-Pumpkin-vs-Papo.md) | 与 REF/Papo（Paper fork）的存档系统逐维对比：触发节奏、写盘管线、格式、实体语义、失败/关停、配置面、互操作性 | 双侧保存架构对比图 |
| [08-存档格式深挖-Pumpkin-vs-Papo.md](08-存档格式深挖-Pumpkin-vs-Papo.md) | 磁盘字节层对比：MCA 逐字节差异、压缩策略、Linear V2/Pump 格式解剖、区块 NBT 逐字段表、实体/POI/level.dat/玩家格式、互操作矩阵、格式风险清单 | 磁盘布局树、Linear V2 结构图 |
| [09-存档系统重写实现记录.md](09-存档系统重写实现记录.md) | 实现记录：MC 版本切至 1.21.11；Papo 兼容 RegionFile（255 扩展/.mcc/oversized/头自愈/原子写/扇区溢出防护）、区块 NBT 全字段保留、玩家/level.dat/POI 原子写、保存管线防丢；230 测试全绿 | — |

## 核心发现（十件事）

1. **双版本同服**：`ClientPlatform` 枚举统一 Java/Bedrock 连接，出站包 `enqueue_packet_editioned<J,B>` 按版本分别序列化；`pumpkin-data` 的 17 个 `*_id_remap` feature 是 ID 翻译的编译期开关。
2. **三执行域**：tokio（网络/IO/插件驱动）+ rayon 全局池（并行模拟）+ 每世界 `ChunkGen` 专用池；`main.rs:44` 的"rayon 不得阻塞 tokio"是全库并发纪律。
3. **单一 tick 真源**：`Server-Ticker` 专用线程 50ms/tick，世界/玩家/实体/方块计划 tick/刷怪全部 rayon 分批并行；网络包在 `Player::tick` 开头消费（≤64/tick）。
4. **红石双路径**：tick 开头 `flush_synced_block_events`（同步方块事件）+ `tick_chunks` 第一步计划 block tick（`world/mod.rs:1513,1963`）。
5. **区块生成 DAG**：`StagedChunkEnum` 11 阶段（Empty→Biomes→…→Lighting→Spawn→Full），`Schedule` 专用线程 + 4 读 1 写 IO 任务 + 生成池；保存支持 anvil/linear/pump 三格式，实体区块按快照整写。
6. **插件即 WASM 组件**：WIT world `pumpkin:plugin@0.1.0`（30+ host import/10+ guest export），宿主与插件两侧类型都从同一契约生成，CI 强制校验不漂移。
7. **事件可否决**：`PluginManager::fire` 先串行 blocking handler（可修改/取消事件）再非 blocking；命令、聊天、包收发等关键路径均有 veto 点。
8. **Brigadier 克隆命令树**：`ArcSwap<CommandDispatcher>` 支持插件装卸时**无锁换树并全体重发**命令数据包；权限谓词在解析期过滤（无权限命令不可见）。
9. **数据即代码**：`assets/*.json` → `tools/pumpkin-codegen` → 73MB 静态 Rust 数据，零运行时解析成本。
10. **生产级健壮性**：clippy deny unwrap/expect/panic；panic→崩溃报告→优雅关停；30s 登录超时、keep-alive、包限速、WASM 沙箱（内存/socket/目录/签名）层层设防。

## 证据约定

所有论断均以 `文件路径:行号` 形式内联标注（如 `server/ticker.rs:20` 指 `crates/pumpkin/src/server/ticker.rs` 第 20 行）。行号基于当前工作区（master @ c31eb836 之后），随代码演进可能漂移。

## 分析方法说明

- 大目录（`entity/` 290 文件、`net/java/play/` 61 处理器、`mob/` 50 文件）采用"全量目录扫描 + 代表文件精读"的采样策略。
- 深度追踪由 3 个并行子代理完成（服务器核心/插件系统/命令配置认证），网络、世界、实体、支撑库由主分析直接完成（子代理因当日用量限额部分失败，未影响覆盖面）。
- `REF/` 目录按要求完全未读取。
