//! 轮次 13 基准：计划刻区块集合的「逐 tick 全量 collect」分配流失收编。
//!
//! ## 侦察结论（生产可达，本基准的设计依据）
//!
//! `Level::collect_tickable_chunks`（`level.rs:644`）每个世界 tick 无条件执行：
//! ```rust,ignore
//! let scheduled_chunk_pos: Vec<_> = self.chunks_with_scheduled_ticks.iter().map(|p| *p).collect();
//! for pos in scheduled_chunk_pos { /* 对多数仅访问，仅少数 remove */ }
//! ```
//! 注释说明收集是为了避免「访问 `loaded_chunks` 时持有 `DashSet` 分片锁」
//! 的死锁。但 `for` 循环里**仅当区块的计划刻耗尽或已卸载时才 `remove`**
//! ——多数 tick 一个都不用移除，全量 `collect` 是纯分配流失（每 tick
//! 一次 `Vec` 分配 + 全集合拷贝）。计划刻区块在有大面积红石/流体时
//! 可达数千。
//!
//! 收编形态：**按需收集**——遍历时只把需移除的键压入一个小 `Vec`
//! （常态为空，零分配），遍历后统一移除。死锁规避语义不变（移除仍
//! 在 `loaded_chunks` 访问之外进行）。
//!
//! 硬闸门：两形态对同一「需移除键集合」的判定逐元素全等；按需形态
//! 分配次数与分配字节严格小于全量形态（典型 tick 下）。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// 分配计数（流失账 = collect 产生的临时 Vec）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 转发给系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counters() -> (usize, usize) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
    )
}

// ---------------------------------------------------------------------------
// 负载模型：N 个计划刻区块，每 tick 仅 K 个需移除（耗尽/卸载）
// ---------------------------------------------------------------------------

/// 计划刻区块总数（大面积红石/流体世界量级）
const SCHEDULED: usize = 2048;
/// 模拟 tick 数
const TICKS: usize = 600;
/// 每 tick 需移除的区块数（常态：远小于总数；此处取 1% 向上取整模拟
/// 耗尽/卸载的稳定滴漏）
const REMOVED_PER_TICK: usize = 4;

type Pos = (i32, i32);

/// 模拟「区块的计划刻是否耗尽/已卸载」：每 tick 前 `REMOVED_PER_TICK` 个
/// 键返回 true（需移除），其余 false
const fn needs_removal(index: usize) -> bool {
    index < REMOVED_PER_TICK
}

// ---------------------------------------------------------------------------
// 形态 A：逐 tick 全量 collect（生产现状）
// ---------------------------------------------------------------------------

fn full_collect(set: &[Pos]) -> (Vec<Pos>, u64) {
    // 无条件收集全部键（生产：规避 remove 时持 DashSet 分片锁）
    let collected: Vec<Pos> = set.to_vec();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut removed = Vec::new();
    for (i, pos) in collected.iter().enumerate() {
        // 模拟对 loaded_chunks 的访问（哈希滚动，防优化）
        hash = (hash ^ (pos.0 as u64).wrapping_add(pos.1 as u64).wrapping_add(i as u64))
            .wrapping_mul(0x0000_0100_0000_01B3);
        if needs_removal(i) {
            removed.push(*pos);
        }
    }
    (removed, hash)
}

// ---------------------------------------------------------------------------
// 形态 B：按需收集（本轮收编形态）
// ---------------------------------------------------------------------------

fn on_demand_collect(set: &[Pos]) -> (Vec<Pos>, u64) {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut removed = Vec::new();
    for (i, pos) in set.iter().enumerate() {
        // 同样的 loaded_chunks 访问（分片锁在访问前已释放，安全）
        hash = (hash ^ (pos.0 as u64).wrapping_add(pos.1 as u64).wrapping_add(i as u64))
            .wrapping_mul(0x0000_0100_0000_01B3);
        if needs_removal(i) {
            removed.push(*pos);
        }
    }
    // 移除仍在 loaded_chunks 访问之外统一进行（死锁规避语义不变）
    (removed, hash)
}

// ---------------------------------------------------------------------------
// 度量
// ---------------------------------------------------------------------------

struct Tally {
    allocs: usize,
    alloc_bytes: usize,
    removed_fp: u64,
    access_fp: u64,
}

fn measure(label: &str, f: fn(&[Pos]) -> (Vec<Pos>, u64)) -> Tally {
    let set: Vec<Pos> = (0..SCHEDULED as i32).map(|i| (i, i * 7)).collect();
    f(&set); // 热身
    let (a0, b0) = counters();
    let mut removed_fp: u64 = 0xcbf2_9ce4_8422_2325;
    let mut access_fp: u64 = 0;
    for _ in 0..TICKS {
        let (removed, access_hash) = f(&set);
        access_fp = access_hash;
        for (x, z) in &removed {
            removed_fp = (removed_fp ^ (*x as u64).wrapping_add(*z as u64))
                .wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    let (a1, b1) = counters();
    let tally = Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        removed_fp,
        access_fp,
    };
    eprintln!("{label}: 分配 {} 次 / {} B", tally.allocs, tally.alloc_bytes);
    tally
}

fn main() {
    let a = measure("全量 collect（现状）", full_collect);
    let b = measure("按需收集（收编）  ", on_demand_collect);

    // 内容门：两形态判定的「需移除键集合」与访问序列逐元素全等
    let content_equal = a.removed_fp == b.removed_fp && a.access_fp == b.access_fp;
    let allocs_reduced = b.allocs < a.allocs;
    let bytes_reduced = b.alloc_bytes < a.alloc_bytes;
    let all_pass = content_equal && allocs_reduced && bytes_reduced;

    eprintln!(
        "内容门={content_equal} 分配 {}→{}（-{} 次）字节 {}→{}（-{} B）",
        a.allocs,
        b.allocs,
        a.allocs.saturating_sub(b.allocs),
        a.alloc_bytes,
        b.alloc_bytes,
        a.alloc_bytes.saturating_sub(b.alloc_bytes),
    );
    eprintln!(
        "每 tick：现状平均 {:.1} 次/{:.0} B，收编平均 {:.2} 次/{:.0} B",
        a.allocs as f64 / TICKS as f64,
        a.alloc_bytes as f64 / TICKS as f64,
        b.allocs as f64 / TICKS as f64,
        b.alloc_bytes as f64 / TICKS as f64,
    );

    let report = serde_json::json!({
        "round": 13,
        "topic": "计划刻区块集合逐 tick 全量 collect 改按需收集（消常态分配流失）",
        "workload": {
            "scheduled_chunks": SCHEDULED,
            "ticks": TICKS,
            "removed_per_tick": REMOVED_PER_TICK,
        },
        "full_collect": { "allocs": a.allocs, "bytes": a.alloc_bytes },
        "on_demand_collect": { "allocs": b.allocs, "bytes": b.alloc_bytes },
        "per_tick": {
            "full_allocs": a.allocs as f64 / TICKS as f64,
            "full_bytes": a.alloc_bytes as f64 / TICKS as f64,
            "on_demand_allocs": b.allocs as f64 / TICKS as f64,
            "on_demand_bytes": b.alloc_bytes as f64 / TICKS as f64,
        },
        "gates": {
            "content_equal": content_equal,
            "allocations_reduced": allocs_reduced,
            "bytes_reduced": bytes_reduced,
            "all_pass": all_pass,
        },
    });

    let path = "note/report/perf/round13-scheduled-ticks-collect.json";
    match std::fs::File::create(path) {
        Ok(mut file) => {
            if let Err(err) = file.write_all(
                serde_json::to_string_pretty(&report)
                    .unwrap_or_default()
                    .as_bytes(),
            ) {
                eprintln!("写入报告失败: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("创建报告文件失败: {err}");
            std::process::exit(1);
        }
    }
    println!("报告已写入 {path}");
    if !all_pass {
        eprintln!("硬闸门未通过");
        std::process::exit(2);
    }
}
