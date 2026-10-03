# 16-CPU 异常占用根因与修复（rayon 全局池窃取空旋）

> 状态：2026-10-03 定位并修复。现象：实际使用时 CPU 占用异常高（1 个 AFK 玩家即
> 持续烧掉约 2.3 个核）。根因：rayon 全局线程池默认拉满逻辑核数，游戏刻流水线
> 每刻发起十余次池级 fork-join，工作线程的「唤醒→窃取空旋→驻留」固定开销在
> Windows GNU 工具链上被放大两个数量级。修复：池容量限到 `(核数/4).clamp(4, 8)`
> + 每刻 fork-join 粗化（小集合改串行）。

## 现象与测量

统一测量口径：取进程 `TotalProcessorTime` 差值除以挂钟时间，得「占用核数」。

| 场景 | rayon 工作线程数 | 实测 CPU |
|---|---|---|
| 空载（0 玩家） | 32（默认=核数） | 0.176 核 |
| 空载 | 4 | 0.030 核 |
| 1 个 AFK bot 挂出生点 | 32 | **2.345 核**（release 产物） |
| 1 个 AFK bot | 4 | 0.120 核 |
| 1 个 AFK bot | 8 | 0.220 核 |

即 32 线程配置下约 2.2 个核是纯空耗；池子缩小后同一工作负载只需 0.12 核。

## 证据链

1. **分线程 CPU 采样**（`sample-cpu.ps1` 按 TID 取 User+Kernel 时间）：发热的
   几乎全是 rayon 全局池的 32 个 `Rayon-Worker-*`（合计 ~2.26 核）；tokio、
   ChunkGen（3 世界 ×16）、Schedule、Server-Ticker 全部接近零。线程角色映射靠
   临时插桩（各池 `start_handler`/`on_thread_start` 打印 TID，事后已全部还原）。
2. **RIP 采样**（自研 `ripsampler`：SuspendThread + GetThreadContext，60 秒
   476224 个样本）：91% 落在 `ntdll!ZwWaitForAlertByThreadId+0x14`（即 parked
   驻留），其余多在 `ZwDelayExecution`（自旋后的退让等待）。说明时间不是花在
   真实计算上，而是花在「等待」路径上。
3. **栈回溯归因**（读 2KB 栈 + addr2line 解析 DWARF，`attribute_stacks.py`
   聚合 30236 个活跃栈）：92.6% 在 `rayon_core::sleep::WorkerThread::steal`
   → `wait_until::<OnceLatch>` → `crossbeam_epoch::default::pin` →
   `Storage::<LocalHandle>::get`（TLS 访问）。即工作线程被唤醒后在窃取循环里
   反复做 epoch pin + TLS 查询，找不到任务再驻留。
4. **分阶段计时**（临时 `diag_prof` 模块，事后已删）：32 线程时 spawn
   0.107 核 + `living_tick` 0.063 核等合计仅 ~0.26 核可被真实工作解释；且同一
   阶段在 4 线程配置下单次调用耗时下降 4-6 倍（如 `spawn/for_chunk`
   15.1µs→2.3µs，`mob/living_tick` 31.6µs→8.2µs）——32 线程的争用把
   「真实工作」本身也放大了数倍。

## 根因

游戏刻流水线（`World::tick`/`tick_chunks`）每刻对玩家、实体、方块实体、
方块刻、流体刻、随机刻、自然生成、inhabited_time 等分别发起一次
`par_iter`/`par_chunks` —— 每次都是一个池级 fork-join：主线程 latch 唤醒
**全部** N 个工作线程，它们发现任务只有寥寥几个（多数集合为空或只有几十个
元素）后又窃取空旋、驻留。每秒 20 刻 × 每刻 10-15 次 ≈ 每秒 200-300 次
全池唤醒。

在 Linux/glibc 上这种空转每次代价很小，但本机是 **x86_64-pc-windows-gnu**：
rust 在该目标上用模拟 TLS（`__emutls_get_address` 每次访问都要查表 +
原子操作），rayon 窃取循环里每轮迭代都要走 TLS 取 `LocalHandle` 和
crossbeam-epoch 的 `pin()`（携带动态链接到 ntdll 的原子/内存屏障）。唤醒-
窃取-驻留的单次成本被放大到微秒级以上，× 32 线程 × 每秒数百次唤醒
≈ 2.2 个核的纯固定开销。

**这正是「默认 = 逻辑核数」策略对「高频小任务」负载的失效场景**：
池子越大，空耗越大；而真实并行工作量（几十个实体的 tick）不足 0.3 核，
根本不需要 32 个线程。

## 排除过的嫌疑

- **网络发包风暴**：bot 侧抓包 60 秒 99167 包 / 65.4 MiB，其中实体移动类
  （51/81/54/52）占 96%，约 1600 包/秒，对 42-80 只生物是原版正常量级；
  广播链路已是「按版本序列化一次 + Bytes 克隆分发」。
- **插件事件 fire_blocking 空转**：无插件时走空 map 快路径，纳秒级。
- **区块重复推送**：17.6 区块/秒量级，且 `ChunkSender` 有编码缓存与 epoch
  去重；keepalive/断线清理链路工作正常（曾怀疑的「幽灵玩家」实为 bot 无
  退出条件，非服务器 bug）。
- **SpawnState 重建**：每刻全量扫实体 ~124µs，真实存在但量级太小。

## 修复内容

1. **限容**（`crates/papokin/src/main.rs`）：全局 rayon 池大小从「默认 =
   逻辑核数」改为 `(核数 / 4).clamp(4, 8)`，并留 `PAPOKIN_RAYON_THREADS`
   环境变量供特大型服务器调高。8 线程对游戏刻并行 + 区块 IO 序列化
   （`rayon::spawn` 走同一池）已绰绰有余。
2. **fork-join 粗化**（`crates/papokin/src/world/mod.rs`）：小集合不再唤醒
   线程池，直接串行；大集合保留并行。阈值都按「并行调度固定开销 > 串行
   执行」的经验点选取：
   - 玩家 tick：`≤2` 串行；
   - 实体过滤：`≤256` 串行；实体批 tick：`≤2×16` 串行；
   - 方块实体：`≤2×16` 串行；
   - 方块/流体/随机刻：`≤2×32` 串行；
   - 自然生成：批次 8→32，`≤2×32` 串行；
   - `inhabited_time` 累加：`≤2×1024` 串行（原来虽有 `with_min_len(1024)`
     限制拆分，但小集合仍走了一次池级 fork-join 调度）。

## 诊断工具链（无管理员权限、windows-gnu 可用）

均留在 `target/cpu-repro/`（不入库）：

- `bot/`：假 MC 1.21.11 客户端（协议 774），可复现负载；带自动退出参数。
- `ripsampler/`：外部 RIP/栈采样器（同用户进程间即可，无需管理员）：
  `ripsampler <pid> <间隔ms> <秒数> <tids|all>`，`RIPSAMPLER_STACKS=1` 时
  回读 2KB 栈并打印 `STACK` 行。
- `attribute_stacks.py`：把运行时地址减模块基址（ASLR）再加镜像基址
  0x140000000 得到文件 VA，批量喂 addr2line 解析后聚合归因。
- `sample-cpu.ps1` / `sample-cpu-rva.ps1`：分线程 CPU 采样（参数是
  `-TargetPid`，`-Pid` 是 PowerShell 保留名）。

踩过的坑备查：pprof 0.14 在 windows-gnu 上不可编译（msvc 专用）；WPR 需
管理员、WPA 读不了 DWARF；stable 工具链用 rust-lld 要 `-C linker-flavor=
ld.lld`（`gnu-lld` flavor 仍不稳定）；addr2line 要的是文件 VA 而非 RVA，
且 Python 子进程里必须用 exe 绝对路径。

## 验证

- 门禁：fmt / clippy -D warnings / cargo test --workspace 全绿。
- 实测（0.3.11 release 产物 + 1 个 AFK bot，同方法同场景）：
  - 稳态（入服 30 秒后取 80 秒窗口）：**0.192 核**，与 8 线程实验组的
    0.22 核一致，较基线 2.345 核下降约 92%；
  - 含入服区块突发全程均摊：0.349 核；
  - 空载：0.037 核（修复前 0.176 核）。
  - 负载等价性：bot 收包量级与基线轮一致（95 秒 90503 包 / 65.4 MiB，
    实体移动包 ~1600/秒）。
