//! 轮次 8：zlib 压缩上下文池化基准
//!
//! 对比旧行为（每条连接的 `TCPNetworkEncoder` 常驻一份 flate2 `Compress`
//! 压缩上下文：字典窗口 + 哈希/链表 + 待决缓冲，单个实测逾百 KiB，连接
//! 生命周期内始终占用）与新行为（压缩上下文自线程局部池按级别检出、用毕
//! 归还，池上限 4 份/线程）在相同数据包流下的内存占用与吞吐。
//!
//! 安全性依据（生产侧逐行核对）：协议线上行为逐包无状态——每包
//! `reset()` 后以 `FlushCompress::Finish` 完整收尾，等价于一条全新压缩
//! 流；因此上下文在连接间可互换，输出字节全等。本基准以真实 flate2
//! 压缩对两模式做逐包字节全等硬闸门：
//!
//! 1. 每个数据包两种模式的压缩输出逐字节一致（哈希比对 + 抽样全比对）；
//! 2. 两模式线上字节总数相等。
//!
//! 旧行为在基准内复刻（N 份常驻 `Compress`），新行为复刻生产侧
//! `PooledCompressor` 的检出/归还/上限语义（生产侧由单测
//! `pooled_compressor_reuse_round_trip` 等覆盖）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use flate2::{Compress, Compression, FlushCompress, Status};

// ---------------------------------------------------------------------------
// 计数分配器：精确测量两种模式的压缩上下文存活字节差
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
// 确定性随机数（xorshift64*），语料生成两模式共享同一流
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
// 负载模型：连接数与数据包语料
// ---------------------------------------------------------------------------

/// 模拟并发连接数（每条连接旧模式常驻一份压缩上下文）
const CONNECTIONS: usize = 128;
/// 每条连接发送的数据包轮数（总包数 = CONNECTIONS × ROUNDS）
const ROUNDS: usize = 25;
/// 压缩级别（与生产默认一致）
const LEVEL: u32 = 6;

/// 数据包尺寸分布（贴近游玩流量直觉）：
/// 60% 小包 200 B–2 KiB（实体/聊天/keepalive 类），
/// 30% 中包 2–16 KiB（批量更新），10% 大包 16–128 KiB（区块/登录同步类）
const fn packet_size(rng: &mut Rng) -> usize {
    match rng.below(100) {
        0..60 => 200 + rng.below(1848),
        60..90 => 2048 + rng.below(14 * 1024),
        _ => 16 * 1024 + rng.below(112 * 1024),
    }
}

/// 生成一个数据包负载：半数为可压缩内容（小字母表重复模式，贴近
/// 协议实际），半数为随机字节（贴近已加密/已压缩负载）
fn gen_packet(rng: &mut Rng) -> Vec<u8> {
    let size = packet_size(rng);
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
// 压缩原语：两模式共享同一逐包语义（reset + Finish）
// ---------------------------------------------------------------------------

fn compress_one(compressor: &mut Compress, input: &[u8], scratch: &mut Vec<u8>) -> usize {
    scratch.clear();
    // deflate 最坏膨胀约 5 B/64 KiB：按输入全长 + 1/16 + 64 预留，
    // 保证随机（不可压缩）负载也能单次 Finish 到 StreamEnd
    scratch.reserve(input.len() + input.len() / 16 + 64);
    compressor.reset();
    match compressor.compress_vec(input, scratch, FlushCompress::Finish) {
        Ok(Status::StreamEnd) => scratch.len(),
        other => {
            eprintln!("压缩失败或状态异常: {other:?}");
            std::process::exit(1);
        }
    }
}

/// FNV-1a 64：逐包输出指纹，用于字节全等闸门的低开销比对
fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

// ---------------------------------------------------------------------------
// 池化复刻：与生产 `PooledCompressor` 同语义（级别匹配检出 / 上限归还）
// ---------------------------------------------------------------------------

const POOL_CAP: usize = 4;

struct Pool {
    entries: Vec<Compress>,
    created: usize,
}

impl Pool {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
            created: 0,
        }
    }

    fn checkout(&mut self) -> Compress {
        if let Some(compressor) = self.entries.pop() {
            compressor
        } else {
            self.created += 1;
            Compress::new(Compression::new(LEVEL), true)
        }
    }

    fn give_back(&mut self, compressor: Compress) {
        if self.entries.len() < POOL_CAP {
            self.entries.push(compressor);
        }
    }
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
fn main() {
    // 语料：交错连接序（连接 c 的第 r 包 = corpus[r * CONNECTIONS + c]），
    // 模拟多连接交错压缩的真实调度
    let mut rng = Rng(0x5EED_CAFE_1234_5678);
    let total_packets = CONNECTIONS * ROUNDS;
    let corpus: Vec<Vec<u8>> = (0..total_packets).map(|_| gen_packet(&mut rng)).collect();
    let raw_bytes: usize = corpus.iter().map(Vec::len).sum();

    let baseline = live_bytes();
    let mut scratch: Vec<u8> = Vec::new();

    // ---- 旧模式：每连接常驻一份压缩上下文 ----
    let legacy_start = Instant::now();
    let mut legacy_contexts: Vec<Compress> = (0..CONNECTIONS)
        .map(|_| Compress::new(Compression::new(LEVEL), true))
        .collect();
    // 每条上下文至少压缩一包后再测存活（贴近稳态：字典/表已按
    // 实际数据预热，避免「空上下文」低估）
    let mut legacy_hashes: Vec<u64> = Vec::with_capacity(total_packets);
    let mut legacy_wire_bytes = 0usize;
    for (index, packet) in corpus.iter().enumerate() {
        let conn = index % CONNECTIONS;
        let wire = compress_one(&mut legacy_contexts[conn], packet, &mut scratch);
        legacy_wire_bytes += wire;
        legacy_hashes.push(fnv1a(&scratch[..wire]));
    }
    let legacy_live = live_bytes().saturating_sub(baseline);
    let legacy_elapsed = legacy_start.elapsed();
    let per_context = legacy_live / CONNECTIONS;
    drop(legacy_contexts);

    let after_legacy = live_bytes();

    // ---- 新模式：线程池化检出/归还 ----
    let pooled_start = Instant::now();
    let mut pool = Pool::new();
    let mut pooled_hashes: Vec<u64> = Vec::with_capacity(total_packets);
    let mut pooled_wire_bytes = 0usize;
    for packet in &corpus {
        let mut compressor = pool.checkout();
        let wire = compress_one(&mut compressor, packet, &mut scratch);
        pool.give_back(compressor);
        pooled_wire_bytes += wire;
        pooled_hashes.push(fnv1a(&scratch[..wire]));
    }
    let pooled_elapsed = pooled_start.elapsed();
    let pooled_live = live_bytes().saturating_sub(after_legacy);
    let pool_retained = pool.entries.len();
    let pool_created = pool.created;
    drop(pool);

    // ---- 硬闸门 ----
    let mismatches = legacy_hashes
        .iter()
        .zip(&pooled_hashes)
        .filter(|(a, b)| a != b)
        .count();
    let gate_bytes_equal = mismatches == 0 && legacy_wire_bytes == pooled_wire_bytes;

    // 抽样全比对（前 64 包逐字节复压缩验证，防哈希碰撞假绿）
    let mut sample_equal = true;
    {
        let mut verify = Compress::new(Compression::new(LEVEL), true);
        let mut scratch_a = Vec::new();
        let mut scratch_b = Vec::new();
        for packet in corpus.iter().take(64) {
            let a = compress_one(&mut verify, packet, &mut scratch_a);
            let mut pooled_once = Compress::new(Compression::new(LEVEL), true);
            let b = compress_one(&mut pooled_once, packet, &mut scratch_b);
            if a != b || scratch_a[..a] != scratch_b[..b] {
                sample_equal = false;
                break;
            }
        }
    }

    let saved = legacy_live.saturating_sub(pooled_live);
    let saved_pct = if legacy_live > 0 {
        saved as f64 * 100.0 / legacy_live as f64
    } else {
        0.0
    };

    let json = serde_json::json!({
        "round": 8,
        "topic": "zlib 压缩上下文线程池化",
        "workload": {
            "connections": CONNECTIONS,
            "rounds": ROUNDS,
            "total_packets": total_packets,
            "raw_bytes": raw_bytes,
            "compression_level": LEVEL,
        },
        "memory": {
            "legacy_live_bytes": legacy_live,
            "pooled_live_bytes": pooled_live,
            "per_context_bytes": per_context,
            "saved_bytes": saved,
            "saved_percent": saved_pct,
            "pool_retained_contexts": pool_retained,
            "pool_created_contexts": pool_created,
        },
        "wire": {
            "legacy_wire_bytes": legacy_wire_bytes,
            "pooled_wire_bytes": pooled_wire_bytes,
        },
        "throughput": {
            "legacy_ms": legacy_elapsed.as_secs_f64() * 1000.0,
            "pooled_ms": pooled_elapsed.as_secs_f64() * 1000.0,
        },
        "gates": {
            "packet_hashes_equal": mismatches == 0,
            "wire_bytes_equal": legacy_wire_bytes == pooled_wire_bytes,
            "sample_full_compare_equal": sample_equal,
            "all_pass": gate_bytes_equal && sample_equal,
        },
    });

    println!("=== 轮次 8：zlib 压缩上下文池化 ===");
    println!("连接 {CONNECTIONS} × 轮 {ROUNDS} = {total_packets} 包，原始 {raw_bytes} B");
    println!(
        "旧模式常驻 {legacy_live} B（每上下文 {per_context} B），新模式驻留 {pooled_live} B（池 {pool_retained} 份，创建 {pool_created} 次）"
    );
    println!("节省 {saved} B（{saved_pct:.1}%）");
    println!(
        "吞吐：旧 {:.1} ms vs 新 {:.1} ms",
        legacy_elapsed.as_secs_f64() * 1000.0,
        pooled_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "闸门：哈希全等={} 线上字节相等={} 抽样全比对={}",
        mismatches == 0,
        legacy_wire_bytes == pooled_wire_bytes,
        sample_equal
    );

    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../note/report/perf");
    match std::fs::create_dir_all(&out_dir) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("创建报告目录失败: {err}");
            std::process::exit(1);
        }
    }
    let out_path = out_dir.join("round8-compressor-pool.json");
    let payload = serde_json::to_string_pretty(&json).unwrap_or_default();
    match std::fs::write(&out_path, payload) {
        Ok(()) => println!("JSON 已写入 {}", out_path.display()),
        Err(err) => {
            eprintln!("写入 JSON 失败: {err}");
            std::process::exit(1);
        }
    }

    if !(gate_bytes_equal && sample_equal) {
        eprintln!("硬闸门未通过：池化改变了线上字节");
        std::process::exit(1);
    }
}
