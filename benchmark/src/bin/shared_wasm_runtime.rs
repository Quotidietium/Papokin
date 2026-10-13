//! 轮次 28 基准：Wasmtime 运行时底座共享化（每插件一套 Engine/缓存
//! 线程/Linker/ticker → 进程级单份 + 引用计数）。
//!
//! 对照形态（`PluginRuntime::new` 加载路径）：
//! - 旧臂（现状）：每插件各建一套——一个 Engine（内部类型注册表
//!   等基础结构）+ 一个缓存工作线程（`Cache::new` 无条件
//!   `thread::spawn`）+ 一份全量宿主 API 的 Linker 注册（982 函数）
//!   + 一个 100ms epoch ticker 定时任务；
//! - 新臂（收编）：进程级共享一份 Engine/Linker/缓存/ticker，每插件
//!   只克隆 Arc 句柄与 ticker 引用计数守卫（零分配）。
//!
//! 实测面与模型面（如实区分）：
//! - **实测**：缓存工作线程（真 std 线程）、epoch ticker（真 tokio
//!   interval 任务，100ms 周期与生产一致）、加载段分配/耗时、稳态
//!   唤醒次数、卸载完整性（join/停表）；
//! - **模型**：Engine 内部结构以固定占用向量建模（结构事实 N→1 为
//!   硬结论，字节量按模型口径）；Linker 以 982 条字符串键注册建模
//!   （生产为 WASI p2/p3 + HTTP + papokin 宿主 API 注册）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 稳态唤醒：旧臂 ≈ N×窗口拍数、新臂 ≈ 窗口拍数（共享 ticker
//!   对每个插件仍以每 100ms 一拍驱动截止判定，语义等价）；
//! - G2 加载段分配：新臂只含共享底座一次性构建（≤ 2×注册数+64），
//!   每插件追加分配为 0（守卫/Arc 克隆零分配）；旧臂每插件重建全套；
//! - G3 峰值驻留：旧臂 ≥ 10× 新臂（N=64，结构上 ≈64×）；
//! - G4 卸载完整性：两臂线程全部 join、守卫计数归零；
//! - G5 停表观测：守卫归零后观测窗口零唤醒。
//!
//! （守卫 respawn 语义由生产单测覆盖，基准不建模重启分支。）

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// 分配计数器（次数 + 毛字节 + 释放字节；净驻留 = 毛 − 释放）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREED_BYTES: AtomicU64 = AtomicU64::new(0);

struct Counting;

// SAFETY: 仅转发 System 并计数，不改动布局与返回值语义。
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(
            u64::try_from(layout.size()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(
            u64::try_from(layout.size()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn counters() -> (u64, u64, u64) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
        FREED_BYTES.load(Ordering::Relaxed),
    )
}

fn retained() -> u64 {
    let (_, alloc, freed) = counters();
    alloc.saturating_sub(freed)
}

/// 阶段读数：分配次数增量 + 净驻留增量。
type Delta = (u64, u64);

fn delta_counters() -> (u64, u64) {
    let (a0, _, _) = counters();
    let r0 = retained();
    (a0, r0)
}

fn stage_delta(start: (u64, u64)) -> Delta {
    let (a1, _, _) = counters();
    let r1 = retained();
    (a1 - start.0, r1.saturating_sub(start.1))
}

const fn verdict(pass: bool) -> &'static str {
    if pass { "PASS" } else { "FAIL" }
}

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 插件数（典型大服插件规模）。
const N_PLUGINS: usize = 64;
/// Linker 注册的宿主函数数（papokin 宿主 API 面，含 WASI 族）。
const REGISTRATIONS: usize = 982;
/// epoch ticker 周期（与生产 `EPOCH_TICKER_INTERVAL_MS` 一致）。
const TICKER_INTERVAL_MS: u64 = 100;
/// 稳态观测窗口（≥ 12 拍）。
const STEADY_WINDOW_MS: u64 = 1250;
/// 卸载停表观测窗口（应零唤醒）。
const TEARDOWN_WINDOW_MS: u64 = 350;

/// 缓存工作线程模型：真 std 线程驻留信道（对应生产 `Cache::new`
/// 无条件 `thread::spawn` 的 worker）。`join` 经内部互斥锁以
/// `&self` 完成（新臂底座装在 `Arc` 内）。
struct CacheWorker {
    state: Mutex<WorkerState>,
}

struct WorkerState {
    tx: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl CacheWorker {
    fn spawn() -> Option<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::Builder::new()
            .spawn(move || while rx.recv().is_ok() {})
            .ok()?;
        Some(Self {
            state: Mutex::new(WorkerState {
                tx: Some(tx),
                handle: Some(handle),
            }),
        })
    }
    fn join(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(state.tx.take()); // 通知线程退出
        state.handle.take().is_some_and(|h| h.join().is_ok())
    }
}

/// Engine 内部结构模型：固定占用的类型注册表缓冲（结构事实是
/// N 份 → 1 份；字节量按模型口径，见文件头「模型面」声明）。
#[allow(dead_code)] // 字节量按模型口径计入驻留，读路径无意义
struct ModelEngine {
    registry: Vec<u64>,
}

impl ModelEngine {
    fn new() -> Self {
        Self {
            registry: vec![0; 4096],
        }
    }
}

/// Linker 模型：982 条字符串键注册（生产为 WASI p2/p3 + HTTP +
/// papokin 宿主 API 的全量 `add_to_linker` 注册面）。
#[allow(dead_code)] // 注册表按模型口径计入驻留
struct ModelLinker {
    functions: HashMap<Box<str>, u64>,
}

impl ModelLinker {
    fn new() -> Self {
        let mut functions = HashMap::with_capacity(REGISTRATIONS);
        for i in 0..REGISTRATIONS {
            functions.insert(format!("host-fn-{i:04}").into_boxed_str(), i as u64);
        }
        Self { functions }
    }
}

/// 旧臂的单插件底座：各建一套 Engine/缓存线程/Linker + 独立 ticker。
#[allow(dead_code)] // 底座成员按生产结构保真，守卫字段由 Drop 语义持有
struct OldPluginBase {
    engine: Arc<ModelEngine>,
    cache_worker: Option<CacheWorker>,
    linker: Arc<ModelLinker>,
    ticker: tokio::task::AbortHandle,
    /// 本插件专属 epoch 计数（旧形态每 Engine 一个计数器）。
    epoch: Arc<AtomicU64>,
}

/// 新臂的共享底座：全进程一份，守卫计数随插件增减。
#[allow(dead_code)] // 同 OldPluginBase：保真持有，经 Drop 释放
struct SharedBase {
    engine: Arc<ModelEngine>,
    cache_worker: Option<CacheWorker>,
    linker: Arc<ModelLinker>,
    ticker: tokio::task::AbortHandle,
    epoch: Arc<AtomicU64>,
    live: Mutex<usize>,
}

/// 共享底座的守卫：Drop 递减计数，最后一个释放时中止 ticker。
struct TickerGuard {
    base: Arc<SharedBase>,
}

impl Drop for TickerGuard {
    fn drop(&mut self) {
        let mut live = self
            .base
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *live = live.saturating_sub(1);
        if *live == 0 {
            self.base.ticker.abort();
        }
    }
}

fn spawn_ticker(epoch: Arc<AtomicU64>) -> tokio::task::AbortHandle {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICKER_INTERVAL_MS));
        loop {
            interval.tick().await;
            epoch.fetch_add(1, Ordering::Relaxed);
        }
    })
    .abort_handle()
}

// ---------------------------------------------------------------------------
// 旧臂：每插件一套底座
// ---------------------------------------------------------------------------

struct OldLoadOutcome {
    bases: Vec<OldPluginBase>,
    load: Delta,
    ns: u128,
}

fn old_arm_load() -> OldLoadOutcome {
    let c0 = delta_counters();
    let t0 = Instant::now();
    let mut bases = Vec::with_capacity(N_PLUGINS);
    for _ in 0..N_PLUGINS {
        let epoch = Arc::new(AtomicU64::new(0));
        let ticker = spawn_ticker(Arc::clone(&epoch));
        bases.push(OldPluginBase {
            engine: Arc::new(ModelEngine::new()),
            cache_worker: CacheWorker::spawn(),
            linker: Arc::new(ModelLinker::new()),
            ticker,
            epoch,
        });
    }
    let ns = t0.elapsed().as_nanos();
    OldLoadOutcome {
        bases,
        load: stage_delta(c0),
        ns,
    }
}

// ---------------------------------------------------------------------------
// 新臂：共享一份底座 + 守卫
// ---------------------------------------------------------------------------

struct NewLoadOutcome {
    base: Arc<SharedBase>,
    guards: Vec<TickerGuard>,
    load: Delta,
    ns: u128,
}

fn new_arm_load() -> NewLoadOutcome {
    let c0 = delta_counters();
    let t0 = Instant::now();
    let epoch = Arc::new(AtomicU64::new(0));
    let base = Arc::new(SharedBase {
        engine: Arc::new(ModelEngine::new()),
        cache_worker: CacheWorker::spawn(),
        linker: Arc::new(ModelLinker::new()),
        ticker: spawn_ticker(Arc::clone(&epoch)),
        epoch,
        live: Mutex::new(0),
    });
    let mut guards = Vec::with_capacity(N_PLUGINS);
    for _ in 0..N_PLUGINS {
        // 守卫获取：计数 +1 即可（共享 ticker 已随底座启动，对应
        // 生产 acquire 的「首个守卫启动任务」分支）。
        *base
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        guards.push(TickerGuard {
            base: Arc::clone(&base),
        });
    }
    let ns = t0.elapsed().as_nanos();
    NewLoadOutcome {
        base,
        guards,
        load: stage_delta(c0),
        ns,
    }
}

/// 段 1：加载段（建底座，两臂），G2/G3。
fn load_segment(gates: &mut bool) -> (OldLoadOutcome, NewLoadOutcome) {
    let old = old_arm_load();
    println!(
        "加载段    旧 {:>9} allocs / +{:<9} B / {:>11} ns",
        old.load.0, old.load.1, old.ns
    );
    let new = new_arm_load();
    println!(
        "加载段    新 {:>9} allocs / +{:<9} B / {:>11} ns",
        new.load.0, new.load.1, new.ns
    );

    // G2：新臂加载分配只含共享底座的一次性构建（982 注册 ×
    // （键 String + Box<str>）+ 基础结构），每插件追加为 0——
    // 上限按 2×注册数 + 64 宽余量钉死。
    let g2 = new.load.0 <= 2 * REGISTRATIONS as u64 + 64;
    println!(
        "G2 新臂加载分配 ≤ 2×{REGISTRATIONS}+64（仅共享底座一次性构建；实际 {}）→ {}",
        new.load.0,
        verdict(g2)
    );
    *gates &= g2;

    // G3：旧臂峰值驻留 ≥ 10× 新臂（结构上 ≈ {N_PLUGINS}×）
    let (old_peak, new_peak) = (old.load.1, new.load.1);
    let g3 = old_peak >= new_peak.saturating_mul(10);
    println!(
        "G3 峰值驻留 旧 ≥ 10×新（{old_peak} ≥ 10×{new_peak}）→ {}",
        verdict(g3)
    );
    *gates &= g3;
    (old, new)
}

/// 段 2：稳态唤醒窗口（两臂底座全存活），G1。
async fn steady_segment(old: &OldLoadOutcome, new: &NewLoadOutcome, gates: &mut bool) {
    let steady_start_old: u64 = old
        .bases
        .iter()
        .map(|b| b.epoch.load(Ordering::Relaxed))
        .sum();
    let steady_start_new = new.base.epoch.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(STEADY_WINDOW_MS)).await;
    let old_total: u64 = old
        .bases
        .iter()
        .map(|b| b.epoch.load(Ordering::Relaxed))
        .sum();
    let new_total = new.base.epoch.load(Ordering::Relaxed);
    let (old_wake, new_wake) = (old_total - steady_start_old, new_total - steady_start_new);
    println!(
        "\n稳态窗口    旧 {old_wake:>7} 次唤醒    新 {new_wake:>5} 次唤醒（{STEADY_WINDOW_MS}ms）"
    );
    // G1：旧 ≈ N×拍数、新 ≈ 拍数（每拍 100ms；首拍立即触发，
    // 1250ms 窗口理论 13 拍，容忍调度抖动）
    let expected_ticks = STEADY_WINDOW_MS / TICKER_INTERVAL_MS + 1;
    let g1 = old_wake >= N_PLUGINS as u64 * (expected_ticks - 2)
        && old_wake <= N_PLUGINS as u64 * (expected_ticks + 2)
        && new_wake >= expected_ticks - 2
        && new_wake <= expected_ticks + 2;
    println!(
        "G1 旧臂 ≈ {N_PLUGINS}×{expected_ticks} 拍、新臂 ≈ {expected_ticks} 拍（共享 ticker 每插件语义等价）→ {}",
        verdict(g1)
    );
    *gates &= g1;
}

/// 段 3：卸载（旧臂逐插件拆底座；新臂守卫逐个释放），G4。
fn teardown_segment(old: OldLoadOutcome, new: &mut NewLoadOutcome, gates: &mut bool) {
    let teardown_t0 = Instant::now();
    let mut joined = 0usize;
    for base in old.bases {
        base.ticker.abort();
        if base.cache_worker.as_ref().is_some_and(CacheWorker::join) {
            joined += 1;
        }
    }
    let old_teardown_ns = teardown_t0.elapsed().as_nanos();
    let old_threads_ok = joined == N_PLUGINS;

    let teardown_t0 = Instant::now();
    new.guards.clear();
    let new_teardown_ns = teardown_t0.elapsed().as_nanos();
    // 共享底座的缓存线程随守卫归零一并停掉
    let new_live = *new
        .base
        .live
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let new_worker_joined = new
        .base
        .cache_worker
        .as_ref()
        .is_some_and(CacheWorker::join);
    println!(
        "\n卸载段    旧 join {joined}/{N_PLUGINS} 线程 / {old_teardown_ns:>9} ns    新 join 1/1 线程 / {new_teardown_ns:>9} ns（live={new_live}）"
    );
    let g4 = old_threads_ok && new_worker_joined && new_live == 0;
    println!(
        "G4 卸载完整性（线程全 join、守卫计数归零）→ {}",
        verdict(g4)
    );
    *gates &= g4;
}

/// 段 4：停表观测（全部 ticker 已中止，窗口内零唤醒），G5。
async fn pause_segment(new: &NewLoadOutcome, gates: &mut bool) {
    let pause_start = new.base.epoch.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(TEARDOWN_WINDOW_MS)).await;
    let g5 = new.base.epoch.load(Ordering::Relaxed) == pause_start;
    println!(
        "G5 停表观测：守卫归零后 {TEARDOWN_WINDOW_MS}ms 零唤醒 → {}",
        verdict(g5)
    );
    *gates &= g5;
}

async fn run_bench() -> bool {
    println!("== 轮次 28 双臂基准：Wasmtime 运行时底座共享化 ==");
    println!(
        "负载：{N_PLUGINS} 插件 ×（Engine 模型 + 缓存线程 + {REGISTRATIONS} 函数 Linker + 100ms ticker）\n"
    );

    let mut gates_pass = true;
    let (old, mut new) = load_segment(&mut gates_pass);
    steady_segment(&old, &new, &mut gates_pass).await;
    teardown_segment(old, &mut new, &mut gates_pass);
    pause_segment(&new, &mut gates_pass).await;

    // 守卫 respawn 语义（归零后再获取应重启定时任务）由生产
    // 单测 `ticker_refcount_starts_stops_and_respawns_shared_task`
    // 覆盖；基准模型的守卫不持有任务句柄、不建模重启分支。
    drop(new.base);
    gates_pass
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap_or_else(|_| std::process::exit(101));

    let gates_pass = runtime.block_on(run_bench());
    println!(
        "\n门禁汇总：{}",
        if gates_pass {
            "全部 PASS"
        } else {
            "存在 FAIL（见上）"
        }
    );
    std::process::exit(i32::from(!gates_pass));
}
