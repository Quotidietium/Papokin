//! 轮次 17 基准：按盒实体/玩家查询（`get_entities_at_box` 一族）逐调用
//! 分配流失的量化与收编对照。
//!
//! ## 侦察结论（生产可达性已验证）
//!
//! `World::get_entities_at_box` 每次调用新建 `Vec<Arc<dyn EntityBase>>`
//! 返回；桶索引路径内部 `ChunkedEntityIndex::query` 另建候选 Vec，再
//! `filter+collect` 出结果 Vec——单次调用最多三次堆分配。`get_players_at_box`
//! 为一次 `filter+cloned+collect`。调用方覆盖每 tick 无条件执行的热路径：
//!
//! - **漏斗方块实体**（每漏斗每 tick 搜索上方掉落物，自动化装置可达数千）；
//! - **实体推挤/搭载**（`entity/mod.rs` 每实体每 tick 3+2 次盒查询）；
//! - **掉落物合并**（每掉落物每 tick）、**弹射物实体命中**（每飞行弹射物
//!   每 tick）、**区域效果云/潜影弹/喷溅药水/唤魔者尖牙**（每 tick）；
//! - **压力板/绊线/比较器/探测铁轨/发射器**（红石刻高频，且多数只需
//!   `is_empty`/`len` 谓词）。
//!
//! 收编形态：① `query_into` 重填 + 结果 Vec `retain` 原地过滤（单缓冲）；
//! ② 迭代消费调用点改线程局部暂存 + `with_*_at_box` 闭包门面；
//! ③ 谓词调用点（`is_empty`）改 `has_*_at_box` 早退零分配。本基准量化
//! 三形态分配账，并以内容指纹作语义等价硬闸门。
//!
//! 硬闸门：两形态产出（候选内容指纹、谓词结果指纹）逐元素全等；复用
//! 形态稳态分配次数与字节严格小于逐调用新建。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// 分配计数
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
// 负载参数
// ---------------------------------------------------------------------------

/// 模拟 tick 数
const TICKS: usize = 600;
/// 每 tick 实体盒查询次数（漏斗 2000 + 推挤 400 + 掉落物 200 + 弹射物 50
/// + 区域效果/红石迭代 350 ≈ 3000）
const ENTITY_QUERIES: usize = 3000;
/// 每 tick 玩家盒查询次数（推挤 200 + 谓词/其他 200）
const PLAYER_QUERIES: usize = 400;
/// 每 tick 谓词（`is_empty`）查询次数（压力板/绊线/发射器等）
const PREDICATE_QUERIES: usize = 500;
/// 单次查询命中候选数上界（小盒常态 0-8）
const MAX_CANDIDATES: usize = 8;

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 命中候选占位：`Arc` 指针对 + 实体 id，16 字节
#[derive(Clone, Copy)]
struct Candidate {
    id: u64,
    token: u64,
}

/// 确定性生成第 q 次查询第 tick 的候选数（0-8，偏少命中为常态）
const fn candidate_count(q: usize, tick: usize) -> usize {
    (q * 7 + tick * 3) % (MAX_CANDIDATES + 1)
}

const fn candidate(q: usize, tick: usize, i: usize) -> Candidate {
    let base = (q as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((tick as u64) << 8)
        .wrapping_add(i as u64);
    Candidate {
        id: base,
        token: base.rotate_left(27),
    }
}

const fn fp_mix(fp: u64, v: u64) -> u64 {
    (fp ^ v).wrapping_mul(0x0000_0100_0000_01B3)
}

// ---------------------------------------------------------------------------
// 形态 A：逐调用新建（生产现状）
// ---------------------------------------------------------------------------

/// 模拟桶索引路径：`query` 新建候选 Vec，`filter+collect` 新建结果 Vec；
/// 约半数候选被相交过滤保留
fn entity_query_fresh(q: usize, tick: usize, fp: &mut u64) {
    let count = candidate_count(q, tick);
    // query 内部候选 Vec
    let mut raw: Vec<Candidate> = Vec::new();
    for i in 0..count {
        raw.push(candidate(q, tick, i));
    }
    // filter + collect 结果 Vec
    let result: Vec<Candidate> = raw
        .into_iter()
        .filter(|c| (c.id & 1) == (q as u64 & 1))
        .collect();
    for c in &result {
        *fp = fp_mix(*fp, c.id ^ c.token);
    }
}

/// 模拟玩家盒查询：`filter+cloned+collect` 一次分配
fn player_query_fresh(q: usize, tick: usize, fp: &mut u64) {
    let count = candidate_count(q, tick) % 3; // 玩家盒命中更少
    let result: Vec<Candidate> = (0..count).map(|i| candidate(q, tick, i)).collect();
    for c in &result {
        *fp = fp_mix(*fp, c.id ^ c.token);
    }
}

/// 模拟谓词调用点：仍走全量收集后 `is_empty`（生产现状）
#[allow(clippy::needless_collect)] // 刻意建模生产「收集后判空」的浪费形态
fn predicate_query_fresh(q: usize, tick: usize, fp: &mut u64) {
    let count = candidate_count(q, tick);
    let mut raw: Vec<Candidate> = Vec::new();
    for i in 0..count {
        raw.push(candidate(q, tick, i));
    }
    let result: Vec<Candidate> = raw.into_iter().filter(|c| (c.id & 1) == 0).collect();
    *fp = fp_mix(*fp, u64::from(!result.is_empty()));
}

fn per_tick_fresh(tick: usize) -> (u64, u64) {
    let mut content_fp = 0xcbf2_9ce4_8422_2325u64;
    let mut predicate_fp = 0xcbf2_9ce4_8422_2325u64;
    for q in 0..ENTITY_QUERIES {
        entity_query_fresh(q, tick, &mut content_fp);
    }
    for q in 0..PLAYER_QUERIES {
        player_query_fresh(q, tick, &mut content_fp);
    }
    for q in 0..PREDICATE_QUERIES {
        predicate_query_fresh(q, tick, &mut predicate_fp);
    }
    (content_fp, predicate_fp)
}

// ---------------------------------------------------------------------------
// 形态 B：跨调用复用 + 谓词早退（本轮收编形态）
// ---------------------------------------------------------------------------

struct ReusedQuery {
    buf: Vec<Candidate>,
}

impl ReusedQuery {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(MAX_CANDIDATES),
        }
    }

    /// 模拟 `query_into` 重填 + `retain` 原地过滤（单缓冲）
    fn entity_query_reused(&mut self, q: usize, tick: usize, fp: &mut u64) {
        let count = candidate_count(q, tick);
        self.buf.clear();
        self.buf.extend((0..count).map(|i| candidate(q, tick, i)));
        self.buf.retain(|c| (c.id & 1) == (q as u64 & 1));
        for c in &self.buf {
            *fp = fp_mix(*fp, c.id ^ c.token);
        }
    }

    fn player_query_reused(&mut self, q: usize, tick: usize, fp: &mut u64) {
        let count = candidate_count(q, tick) % 3;
        self.buf.clear();
        self.buf.extend((0..count).map(|i| candidate(q, tick, i)));
        for c in &self.buf {
            *fp = fp_mix(*fp, c.id ^ c.token);
        }
    }

    /// 模拟 `has_*_at_box`：逐候选早退，全程零分配
    fn predicate_query_reused(q: usize, tick: usize, fp: &mut u64) {
        let count = candidate_count(q, tick);
        let mut any = false;
        for i in 0..count {
            let c = candidate(q, tick, i);
            if (c.id & 1) == 0 {
                any = true;
                break;
            }
        }
        *fp = fp_mix(*fp, u64::from(any));
    }
}

fn per_tick_reused(scratch: &mut ReusedQuery, tick: usize) -> (u64, u64) {
    let mut content_fp = 0xcbf2_9ce4_8422_2325u64;
    let mut predicate_fp = 0xcbf2_9ce4_8422_2325u64;
    for q in 0..ENTITY_QUERIES {
        scratch.entity_query_reused(q, tick, &mut content_fp);
    }
    for q in 0..PLAYER_QUERIES {
        scratch.player_query_reused(q, tick, &mut content_fp);
    }
    for q in 0..PREDICATE_QUERIES {
        ReusedQuery::predicate_query_reused(q, tick, &mut predicate_fp);
    }
    (content_fp, predicate_fp)
}

// ---------------------------------------------------------------------------
// 度量
// ---------------------------------------------------------------------------

struct Tally {
    allocs: usize,
    alloc_bytes: usize,
    content_fp: u64,
    predicate_fp: u64,
}

fn measure_fresh() -> Tally {
    per_tick_fresh(0); // 热身
    let (a0, b0) = counters();
    let (mut cfp, mut pfp) = (0, 0);
    for tick in 0..TICKS {
        let (c, p) = per_tick_fresh(tick);
        cfp = c;
        pfp = p;
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        content_fp: cfp,
        predicate_fp: pfp,
    }
}

fn measure_reused() -> Tally {
    let mut scratch = ReusedQuery::new();
    per_tick_reused(&mut scratch, 0); // 热身（完成首次 grow）
    let (a0, b0) = counters();
    let (mut cfp, mut pfp) = (0, 0);
    for tick in 0..TICKS {
        let (c, p) = per_tick_reused(&mut scratch, tick);
        cfp = c;
        pfp = p;
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        content_fp: cfp,
        predicate_fp: pfp,
    }
}

fn main() {
    let a = measure_fresh();
    let b = measure_reused();

    // 内容门：候选内容与谓词结果指纹逐元素全等（两形态语义等价）
    let content_equal = a.content_fp == b.content_fp && a.predicate_fp == b.predicate_fp;
    let allocs_reduced = b.allocs < a.allocs;
    let bytes_reduced = b.alloc_bytes < a.alloc_bytes;
    let all_pass = content_equal && allocs_reduced && bytes_reduced;

    eprintln!(
        "逐调用新建：分配 {} 次 / {} B（每 tick {:.1} 次 / {:.0} B）",
        a.allocs,
        a.alloc_bytes,
        a.allocs as f64 / TICKS as f64,
        a.alloc_bytes as f64 / TICKS as f64
    );
    eprintln!(
        "复用+谓词早退：分配 {} 次 / {} B（每 tick {:.2} 次 / {:.0} B）",
        b.allocs,
        b.alloc_bytes,
        b.allocs as f64 / TICKS as f64,
        b.alloc_bytes as f64 / TICKS as f64
    );
    eprintln!(
        "内容门={content_equal} 分配 -{} 次 / -{} B",
        a.allocs.saturating_sub(b.allocs),
        a.alloc_bytes.saturating_sub(b.alloc_bytes),
    );

    let report = serde_json::json!({
        "round": 17,
        "topic": "按盒实体/玩家查询跨调用复用（query_into 重填 + retain 原地过滤 + 线程局部暂存 + 谓词早退零分配）",
        "workload": {
            "ticks": TICKS,
            "entity_queries": ENTITY_QUERIES,
            "player_queries": PLAYER_QUERIES,
            "predicate_queries": PREDICATE_QUERIES,
            "max_candidates": MAX_CANDIDATES,
        },
        "per_call_fresh": { "allocs": a.allocs, "bytes": a.alloc_bytes },
        "reused_buffers": { "allocs": b.allocs, "bytes": b.alloc_bytes },
        "per_tick": {
            "fresh_allocs": a.allocs as f64 / TICKS as f64,
            "fresh_bytes": a.alloc_bytes as f64 / TICKS as f64,
            "reused_allocs": b.allocs as f64 / TICKS as f64,
            "reused_bytes": b.alloc_bytes as f64 / TICKS as f64,
        },
        "gates": {
            "content_equal": content_equal,
            "allocations_reduced": allocs_reduced,
            "bytes_reduced": bytes_reduced,
            "all_pass": all_pass,
        },
    });

    let path = "note/report/perf/round17-box-query-reuse.json";
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
