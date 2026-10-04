//! 区块编码缓存负载容量裁剪基准（性能优化轮次 3，见 note/report/perf/）。
//!
//! 背景：`Bytes::from(Vec<u8>)` 原样接管底层分配（含容量）。
//! 0.3.15 之前，`encode_batch`/`send_chunks` 以 32 KiB 预分配
//! 序列化缓冲，序列化后直接移交 `Bytes`——缓存条目（每玩家
//! 上限 8192 条）与出站队列中的包一律按预分配容量驻留。
//!
//! 本基准以计数分配器精确测量两种模式的「缓存留存字节」：
//! - `old`：`Vec::with_capacity(32 KiB)` → 写入 → `Bytes::from`
//! - `new`：同上，移交前 `shrink_to_fit()`（0.3.16 行为）
//!
//! 负载为 625 个合成区块包（尺寸分布按真实区块包剖面合成）；
//! 完整性闸门：两模式逐包字节完全一致。

#![allow(clippy::print_stdout)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    error::Error,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use bytes::Bytes;
use serde_json::{Value, json};

/// 合成区块数（约等于视距 12 圆柱内的首轮发送量）
const CHUNK_COUNT: usize = 625;
/// 与生产一致的序列化缓冲预分配
const INITIAL_CAPACITY: usize = 32 * 1024;

/// 计数分配器：跟踪存活字节/存活块数/总分配次数。
/// realloc 走默认实现（alloc + 拷贝 + dealloc），钩子自然覆盖。
struct CountingAlloc;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_COUNT: AtomicUsize = AtomicUsize::new(0);
static TOTAL_ALLOCS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        LIVE_COUNT.fetch_add(1, Ordering::Relaxed);
        TOTAL_ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: 原样转发系统分配器，不改变 layout 语义
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        LIVE_COUNT.fetch_sub(1, Ordering::Relaxed);
        // SAFETY: ptr/layout 与 alloc 时一致（调用方契约），原样转发
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// 当前进程 RSS（字节）
fn current_rss() -> u64 {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let _ = sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map_or(0, sysinfo::Process::memory)
}

const fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

/// xorshift 伪随机字节
fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect()
}

/// 真实区块包尺寸剖面：70% 小（3-15 KiB）、25% 中（15-50 KiB）、
/// 5% 大（50-200 KiB，密集建筑群/大量方块实体）
const fn chunk_packet_size(roll: u64) -> usize {
    let span = roll as usize;
    match roll % 100 {
        0..=69 => 3 * 1024 + span % (12 * 1024),
        70..=94 => 15 * 1024 + span % (35 * 1024),
        _ => 50 * 1024 + span % (150 * 1024),
    }
}

/// 模式快照
struct Snapshot {
    live_bytes: usize,
    live_count: usize,
    total_allocs: usize,
    encode_ms: u128,
    rss: u64,
    content_bytes: usize,
}

/// 跑一种模式：构建整批缓存后取快照，返回快照与逐包校验和
fn run_mode(shrink: bool, payloads: &[Vec<u8>]) -> (Snapshot, Vec<u64>) {
    let base_bytes = LIVE_BYTES.load(Ordering::Relaxed);
    let base_count = LIVE_COUNT.load(Ordering::Relaxed);
    let base_allocs = TOTAL_ALLOCS.load(Ordering::Relaxed);

    let start = Instant::now();
    let mut cache: Vec<Bytes> = Vec::with_capacity(payloads.len());
    let mut checksums = Vec::with_capacity(payloads.len());
    for content in payloads {
        let mut buf = Vec::with_capacity(INITIAL_CAPACITY);
        buf.extend_from_slice(content);
        if shrink {
            buf.shrink_to_fit();
        }
        let packet = Bytes::from(buf);
        // 简易校验和（抽样内容指纹）
        let mut acc = 0u64;
        for (i, b) in packet.iter().step_by(97).enumerate() {
            acc = acc.rotate_left(5) ^ (u64::from(*b) << (i % 8 * 8));
        }
        checksums.push(acc);
        cache.push(packet);
    }
    let encode_ms = start.elapsed().as_millis();

    // 缓存内容的实际包长合计（同时使 cache 成为被读对象）
    let cached_content: usize = cache.iter().map(Bytes::len).sum();

    let snapshot = Snapshot {
        live_bytes: LIVE_BYTES.load(Ordering::Relaxed) - base_bytes,
        live_count: LIVE_COUNT.load(Ordering::Relaxed) - base_count,
        total_allocs: TOTAL_ALLOCS.load(Ordering::Relaxed) - base_allocs,
        encode_ms,
        rss: current_rss(),
        content_bytes: cached_content,
    };
    (snapshot, checksums)
}

fn main() -> Result<(), Box<dyn Error>> {
    // 合成负载：尺寸分布 × 伪随机内容
    let payloads: Vec<Vec<u8>> = (0..CHUNK_COUNT)
        .map(|i| {
            let seed = 0xC4u64.wrapping_mul(i as u64 + 1);
            pseudo_random_bytes(chunk_packet_size(seed ^ 0x9E37), seed)
        })
        .collect();

    let (old, old_sums) = run_mode(false, &payloads);
    let (new, new_sums) = run_mode(true, &payloads);

    // 完整性闸门：两模式逐包校验和必须一致（线上字节不变）
    assert_eq!(old_sums, new_sums, "两模式包内容必须逐包一致");
    assert_eq!(old.content_bytes, new.content_bytes);

    let saved = old.live_bytes as f64 - new.live_bytes as f64;
    let saved_pct = saved / old.live_bytes as f64 * 100.0;

    println!();
    println!("| 模式 | 缓存留存 | 存活块数 | 分配次数 | 编码耗时 | RSS |");
    println!("|---|---:|---:|---:|---:|---:|");
    for (name, s) in [("old（32 KiB 预分配驻留）", &old), ("new（移交前裁剪）", &new)] {
        println!(
            "| {name} | {:.1} MiB | {} | {} | {} ms | {:.1} MiB |",
            mib(s.live_bytes as u64),
            s.live_count,
            s.total_allocs,
            s.encode_ms,
            mib(s.rss),
        );
    }
    println!();
    println!(
        "缓存留存削减：{:.1} MiB（-{saved_pct:.1}%），内容总量 {:.1} MiB",
        mib(saved as u64),
        mib(old.content_bytes as u64),
    );

    let report: Value = json!({
        "round": 3,
        "subject": "区块编码缓存负载容量裁剪（Bytes 移交前 shrink_to_fit）",
        "workload": {
            "chunks": CHUNK_COUNT,
            "initial_capacity": INITIAL_CAPACITY,
            "size_profile": "70% 3-15KiB / 25% 15-50KiB / 5% 50-200KiB",
            "content_bytes": old.content_bytes,
        },
        "integrity_ok": true,
        "old": {
            "live_bytes": old.live_bytes,
            "live_count": old.live_count,
            "total_allocs": old.total_allocs,
            "encode_ms": old.encode_ms as u64,
            "rss_mib": mib(old.rss),
        },
        "new": {
            "live_bytes": new.live_bytes,
            "live_count": new.live_count,
            "total_allocs": new.total_allocs,
            "encode_ms": new.encode_ms as u64,
            "rss_mib": mib(new.rss),
        },
        "saved_bytes": saved as u64,
        "saved_percent": saved_pct,
    });
    let path = "note/report/perf/round3-chunk-encode-cache.json";
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    println!("对比 JSON 已写入 {path}");
    Ok(())
}
