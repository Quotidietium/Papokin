//! 轮次 9：压缩资源（zlib 上下文 + 压缩暂存）全局有界池基准
//!
//! 轮次 8 的基准以单线程模型测得压缩上下文池化 -99.2%，但生产的压缩
//! 实际发生在 `frame_batch_maybe_offload` 投出的 `spawn_blocking` 上：
//! tokio 阻塞池按需扩张（上限 512 线程），tick 齐发的批量压缩会让大量
//! 阻塞线程各沾一次压缩——线程局部池在该拓扑下驻留 ≈ 沾过压缩的线程数
//! × 单份大小，满负载时收益归零。本基准以多线程工作分区模型对三种
//! 驻留策略做诚实对比：
//!
//! - **legacy**：每连接常驻一份资源（上下文 + 暂存），与改动前一致；
//! - **thread-local**（轮次 8）：每工作线程池化 ≤4 份；
//! - **global-pool**（轮次 9）：全局跨线程池，驻留封顶 16 份，池空检出
//!   永远新建（上限只约束驻留，不阻塞压缩路径）。
//!
//! 两种线程拓扑：W=16（常态并发）与 W=128（tick 齐发极端，连接数=
//! 线程数）。硬闸门：三模式逐包输出哈希全等、线上字节总数相等。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use flate2::{Compress, Compression, FlushCompress, Status};

// ---------------------------------------------------------------------------
// 计数分配器：精确测量三种策略的资源存活字节差
// ---------------------------------------------------------------------------

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 转发给系统分配器；仅在成功时记账。
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 转发给系统分配器；按新旧尺寸差记账。
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size > layout.size() {
                LIVE_BYTES.fetch_add(new_size - layout.size(), Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn live_bytes() -> usize {
    LIVE_BYTES.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// 确定性随机数（xorshift64*）：语料按 (conn, seq) 派生，三模式逐包一致
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    const fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 模拟并发连接数
const CONNECTIONS: usize = 128;
/// 每连接发送的数据包数（总包数 = `CONNECTIONS` × `PACKETS_PER_CONN`）
const PACKETS_PER_CONN: usize = 300;
/// 压缩级别（与生产默认一致）
const LEVEL: u32 = 6;
/// 暂存驻留上限（与生产 `MAX_RETAINED_SCRATCH` 一致）
const MAX_RETAINED_SCRATCH: usize = 256 * 1024;
/// 全局池驻留上限（与生产 `MAX_POOLED_COMPRESSION_RESOURCES` 一致）
const GLOBAL_POOL_CAP: usize = 16;
/// 线程局部池驻留上限（轮次 8 语义）
const THREAD_POOL_CAP: usize = 4;

/// 数据包尺寸分布（与轮次 8 基准一致）
const fn packet_size(rng: &mut Rng) -> usize {
    match rng.below(100) {
        0..60 => 200 + rng.below(1848),
        60..90 => 2048 + rng.below(14 * 1024),
        _ => 16 * 1024 + rng.below(112 * 1024),
    }
}

/// 由 (conn, seq) 确定性派生一个数据包负载（不落语料库，三模式
/// 各自现算、逐包一致；半数可压缩内容半数随机字节，同轮次 8）
fn gen_packet(conn: usize, seq: usize) -> Vec<u8> {
    let seed = ((conn as u64) * 0x9E37_79B9_7F4A_7C15)
        ^ ((seq as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) | 1);
    let mut rng = Rng(seed);
    let size = packet_size(&mut rng);
    let mut data = Vec::with_capacity(size);
    if rng.below(2) == 0 {
        let alphabet = 2 + rng.below(6);
        let symbols: Vec<u8> = (0..alphabet).map(|_| (rng.next() & 0xFF) as u8).collect();
        for i in 0..size {
            data.push(symbols[i % alphabet.max(1)]);
        }
    } else {
        for _ in 0..size {
            data.push((rng.next() & 0xFF) as u8);
        }
    }
    data
}

// ---------------------------------------------------------------------------
// 压缩资源与逐包压缩原语（与生产语义一致：reset + Finish）
// ---------------------------------------------------------------------------

struct Resources {
    compressor: Compress,
    scratch: Vec<u8>,
}

impl Resources {
    fn new() -> Self {
        Self {
            compressor: Compress::new(Compression::new(LEVEL), true),
            scratch: Vec::new(),
        }
    }

    fn compress_one(&mut self, input: &[u8]) -> u64 {
        self.scratch.clear();
        // 全量预留（len 为 0 时保证容量 ≥ hint），与生产修正后一致
        let hint = input.len() + input.len() / 16 + 64;
        self.scratch.reserve(hint);
        self.compressor.reset();
        match self
            .compressor
            .compress_vec(input, &mut self.scratch, FlushCompress::Finish)
        {
            Ok(Status::StreamEnd) => fnv1a(&self.scratch),
            other => {
                eprintln!("压缩失败或状态异常: {other:?}");
                std::process::exit(1);
            }
        }
    }
}

/// 归还治理（与生产守卫 Drop 一致）：逾 2 倍留存上限收缩回上限
fn give_back_governance(resources: &mut Resources) {
    resources.scratch.clear();
    if resources.scratch.capacity() > 2 * MAX_RETAINED_SCRATCH {
        resources.scratch.shrink_to(MAX_RETAINED_SCRATCH);
    }
}

/// FNV-1a 64：逐包输出指纹
fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

// ---------------------------------------------------------------------------
// 三种驻留策略
// ---------------------------------------------------------------------------

struct ModeResult {
    standing_bytes: usize,
    resources_created: usize,
    wire_hash: u64,
    wire_bytes: u64,
    wall_ms: f64,
}

/// 工作分区：conn c 由工作线程 (c % workers) 顺序处理（静态分区，
/// 等价于池化拓扑下的稳态派发）。返回各线程驻留的资源池。
fn run_workers(
    workers: usize,
    legacy: bool,
    created: &AtomicUsize,
    wire_bytes: &AtomicUsize,
    wire_hash: &AtomicUsize,
) -> Vec<Vec<Resources>> {
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..workers {
            handles.push(scope.spawn(move || {
                let conns: Vec<usize> =
                    (0..CONNECTIONS).filter(|c| c % workers == worker).collect();
                let mut rolling: u64 = 0xcbf2_9ce4_8422_2325;
                let mut bytes = 0usize;
                let mut local_created = 0usize;

                let retained_pool: Vec<Resources> = if legacy {
                    // 每连接常驻一份资源
                    let mut per_conn: Vec<Resources> =
                        conns.iter().map(|_| Resources::new()).collect();
                    local_created += per_conn.len();
                    for (index, &conn) in conns.iter().enumerate() {
                        for seq in 0..PACKETS_PER_CONN {
                            let packet = gen_packet(conn, seq);
                            bytes += packet.len();
                            let hash = per_conn[index].compress_one(&packet);
                            rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
                        }
                    }
                    per_conn
                } else {
                    // 轮次 8：线程局部池 ≤ THREAD_POOL_CAP
                    let mut pool: Vec<Resources> = Vec::new();
                    for &conn in &conns {
                        for seq in 0..PACKETS_PER_CONN {
                            let packet = gen_packet(conn, seq);
                            bytes += packet.len();
                            let mut resources = pool.pop().unwrap_or_else(|| {
                                local_created += 1;
                                Resources::new()
                            });
                            let hash = resources.compress_one(&packet);
                            give_back_governance(&mut resources);
                            if pool.len() < THREAD_POOL_CAP {
                                pool.push(resources);
                            }
                            rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
                        }
                    }
                    pool
                };
                created.fetch_add(local_created, Ordering::Relaxed);
                wire_bytes.fetch_add(bytes, Ordering::Relaxed);
                wire_hash.fetch_add(rolling as usize, Ordering::Relaxed);
                retained_pool
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

fn run_partitioned(workers: usize, legacy: bool) -> ModeResult {
    let baseline = live_bytes();
    let start = Instant::now();
    let created = AtomicUsize::new(0);
    let wire_bytes = AtomicUsize::new(0);
    let wire_hash = AtomicUsize::new(0);

    let retained = run_workers(workers, legacy, &created, &wire_bytes, &wire_hash);
    let standing = live_bytes().saturating_sub(baseline);
    drop(retained);
    ModeResult {
        standing_bytes: standing,
        resources_created: created.load(Ordering::Relaxed),
        wire_hash: wire_hash.load(Ordering::Relaxed) as u64,
        wire_bytes: wire_bytes.load(Ordering::Relaxed) as u64,
        wall_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}

/// 轮次 9 全局池：单互斥 Vec，工作线程共享检出/归还
fn run_global(workers: usize) -> ModeResult {
    let baseline = live_bytes();
    let start = Instant::now();
    let created = AtomicUsize::new(0);
    let wire_bytes = AtomicUsize::new(0);
    let wire_hash = AtomicUsize::new(0);
    let pool: Mutex<Vec<Resources>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for worker in 0..workers {
            let pool = &pool;
            let created = &created;
            let wire_bytes = &wire_bytes;
            let wire_hash = &wire_hash;
            scope.spawn(move || {
                let conns: Vec<usize> =
                    (0..CONNECTIONS).filter(|c| c % workers == worker).collect();
                let mut rolling: u64 = 0xcbf2_9ce4_8422_2325;
                let mut bytes = 0usize;
                let mut local_created = 0usize;
                for &conn in &conns {
                    for seq in 0..PACKETS_PER_CONN {
                        let packet = gen_packet(conn, seq);
                        bytes += packet.len();
                        let mut resources = {
                            let mut guard = pool
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            guard.pop()
                        }
                        .unwrap_or_else(|| {
                            local_created += 1;
                            Resources::new()
                        });
                        let hash = resources.compress_one(&packet);
                        give_back_governance(&mut resources);
                        {
                            let mut guard = pool
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            if guard.len() < GLOBAL_POOL_CAP {
                                guard.push(resources);
                            }
                        }
                        rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
                    }
                }
                created.fetch_add(local_created, Ordering::Relaxed);
                wire_bytes.fetch_add(bytes, Ordering::Relaxed);
                wire_hash.fetch_add(rolling as usize, Ordering::Relaxed);
            });
        }
    });

    let standing = live_bytes().saturating_sub(baseline);
    drop(pool);
    ModeResult {
        standing_bytes: standing,
        resources_created: created.load(Ordering::Relaxed),
        wire_hash: wire_hash.load(Ordering::Relaxed) as u64,
        wire_bytes: wire_bytes.load(Ordering::Relaxed) as u64,
        wall_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

fn main() {
    let mut scenarios = serde_json::json!({});
    let mut all_equal = true;

    for workers in [16usize, 128usize] {
        let legacy = run_partitioned(workers, true);
        let thread_local = run_partitioned(workers, false);
        let global = run_global(workers);

        let hashes_equal =
            legacy.wire_hash == thread_local.wire_hash && legacy.wire_hash == global.wire_hash;
        let bytes_equal =
            legacy.wire_bytes == thread_local.wire_bytes && legacy.wire_bytes == global.wire_bytes;
        all_equal &= hashes_equal && bytes_equal;

        scenarios[format!("w{workers}")] = serde_json::json!({
            "workers": workers,
            "legacy": {
                "standing_bytes": legacy.standing_bytes,
                "resources_created": legacy.resources_created,
                "wall_ms": legacy.wall_ms,
            },
            "thread_local": {
                "standing_bytes": thread_local.standing_bytes,
                "resources_created": thread_local.resources_created,
                "wall_ms": thread_local.wall_ms,
            },
            "global_pool": {
                "standing_bytes": global.standing_bytes,
                "resources_created": global.resources_created,
                "wall_ms": global.wall_ms,
            },
            "gates": {
                "wire_hash_equal": hashes_equal,
                "wire_bytes_equal": bytes_equal,
            },
        });

        println!("=== W={workers} ===");
        println!(
            "legacy       : 驻留 {:>10} B，创建 {:>4} 份，{:>7.1} ms",
            legacy.standing_bytes, legacy.resources_created, legacy.wall_ms
        );
        println!(
            "thread-local : 驻留 {:>10} B，创建 {:>4} 份，{:>7.1} ms",
            thread_local.standing_bytes, thread_local.resources_created, thread_local.wall_ms
        );
        println!(
            "global-pool  : 驻留 {:>10} B，创建 {:>4} 份，{:>7.1} ms",
            global.standing_bytes, global.resources_created, global.wall_ms
        );
        println!("闸门：哈希全等={hashes_equal} 字节相等={bytes_equal}");
    }

    let json = serde_json::json!({
        "round": 9,
        "topic": "压缩资源全局有界池（轮次 8 线程局部池的生产拓扑修正）",
        "workload": {
            "connections": CONNECTIONS,
            "packets_per_conn": PACKETS_PER_CONN,
            "compression_level": LEVEL,
            "global_pool_cap": GLOBAL_POOL_CAP,
            "thread_pool_cap": THREAD_POOL_CAP,
        },
        "scenarios": scenarios,
        "gates": { "all_pass": all_equal },
    });

    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../note/report/perf");
    if let Err(err) = std::fs::create_dir_all(&out_dir) {
        eprintln!("创建报告目录失败: {err}");
        std::process::exit(1);
    }
    let out_path = out_dir.join("round9-compression-resource-pool.json");
    let payload = serde_json::to_string_pretty(&json).unwrap_or_default();
    match std::fs::write(&out_path, payload) {
        Ok(()) => println!("JSON 已写入 {}", out_path.display()),
        Err(err) => {
            eprintln!("写入 JSON 失败: {err}");
            std::process::exit(1);
        }
    }

    if !all_equal {
        eprintln!("硬闸门未通过：三种策略线上输出不一致");
        std::process::exit(1);
    }
}
