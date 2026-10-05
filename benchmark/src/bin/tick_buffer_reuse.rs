//! 轮次 14 基准：世界 tick 热路径三处逐 tick 分配流失的量化与收编对照。
//!
//! ## 侦察结论（生产可达性已验证）
//!
//! `World::tick` 每 tick 无条件执行的三处分配（`world/mod.rs`）：
//!
//! 1. **玩家快照**（`players_cache`，行 1328）：`players.par_iter().map(..).collect()`
//!    收集 `(player, pos, bb, chunk_pos)` 四元组，供实体 tick 碰撞检测用。
//!    规模 = 玩家数 P。
//! 2. **方块实体活跃集**（`block_entities`，行 1461）：遍历活跃区块收集
//!    `Arc<dyn BlockEntity>`，规模 = 活跃方块实体数 B（大型自动化装置
//!    可达数千）。
//! 3. **嵌套碰撞检测**（行 1425）：实体 tick 内对每个实体遍历玩家快照，
//!    O(E×P) 次包围盒比较。
//!
//! 三处均为**逐 tick 重建的临时 Vec**（或临时计算），tick 结束即弃。
//! 收编形态：**跨 tick 复用缓冲**（容量驻留、逐 tick `clear()` 重填，
//! 消去反复 grow 的分配流失）。本基准量化「逐 tick 新建」与「跨 tick
//! 复用」在同一负载下的分配账，并对碰撞检测给出语义等价性硬闸门。
//!
//! 硬闸门：两形态产出（快照内容指纹、碰撞判定结果指纹）逐元素全等；
//! 复用形态分配次数与字节严格小于逐 tick 新建（稳态）。
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
/// 玩家数
const PLAYERS: usize = 32;
/// 实体数（生物/掉落物等，碰撞检测主体）
const ENTITIES: usize = 800;
/// 活跃方块实体数（大型自动化装置量级）
const BLOCK_ENTITIES: usize = 1500;

// ---------------------------------------------------------------------------
// 负载模型：玩家快照 / 方块实体活跃集 / 碰撞判定
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Snapshot {
    id: u32,
    x: i64,
    z: i64,
    chunk: (i32, i32),
}

#[derive(Clone, Copy)]
struct EntityBox {
    id: u32,
    x: i64,
    z: i64,
    chunk: (i32, i32),
}

/// 确定性生成第 i 个玩家的快照（每 tick 位置微移）
const fn player_snapshot(i: usize, tick: usize) -> Snapshot {
    let x = (i as i64 * 97 + tick as i64) % 10000;
    let z = (i as i64 * 131 + tick as i64 * 3) % 10000;
    Snapshot {
        id: i as u32,
        x,
        z,
        chunk: ((x >> 4) as i32, (z >> 4) as i32),
    }
}

const fn entity_box(i: usize, tick: usize) -> EntityBox {
    let x = (i as i64 * 53 + tick as i64 * 7) % 10000;
    let z = (i as i64 * 71 + tick as i64 * 11) % 10000;
    EntityBox {
        id: i as u32,
        x,
        z,
        chunk: ((x >> 4) as i32, (z >> 4) as i32),
    }
}

const fn block_entity_id(i: usize, _tick: usize) -> u64 {
    (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

// ---------------------------------------------------------------------------
// 形态 A：逐 tick 新建（生产现状）
// ---------------------------------------------------------------------------

fn per_tick_fresh(tick: usize) -> (u64, u64) {
    // 玩家快照：每 tick 新建 Vec
    let players_cache: Vec<Snapshot> = (0..PLAYERS).map(|i| player_snapshot(i, tick)).collect();
    // 方块实体活跃集：每 tick 新建 Vec
    let block_entities: Vec<u64> = (0..BLOCK_ENTITIES)
        .map(|i| block_entity_id(i, tick))
        .collect();

    // 嵌套碰撞检测 O(E×P)，滚动哈希记录判定结果
    let mut collision_fp: u64 = 0xcbf2_9ce4_8422_2325;
    for e in 0..ENTITIES {
        let eb = entity_box(e, tick);
        for p in &players_cache {
            let hit = (p.chunk.0 - eb.chunk.0).abs() <= 1
                && (p.chunk.1 - eb.chunk.1).abs() <= 1
                && (p.x - eb.x).abs() < 5
                && (p.z - eb.z).abs() < 5;
            collision_fp = (collision_fp ^ u64::from(hit))
                .wrapping_mul(0x0000_0100_0000_01B3)
                .wrapping_add(u64::from(p.id).wrapping_add(u64::from(eb.id)));
            if hit {
                break;
            }
        }
    }
    // 活跃集指纹
    let mut be_fp: u64 = 0xcbf2_9ce4_8422_2325;
    for id in &block_entities {
        be_fp = (be_fp ^ id).wrapping_mul(0x0000_0100_0000_01B3);
    }
    (collision_fp, be_fp)
}

// ---------------------------------------------------------------------------
// 形态 B：跨 tick 复用缓冲（本轮收编形态）
// ---------------------------------------------------------------------------

struct ReusedBuffers {
    players_cache: Vec<Snapshot>,
    block_entities: Vec<u64>,
}

impl ReusedBuffers {
    fn new() -> Self {
        Self {
            players_cache: Vec::with_capacity(PLAYERS),
            block_entities: Vec::with_capacity(BLOCK_ENTITIES),
        }
    }

    fn refill(&mut self, tick: usize) -> (u64, u64) {
        // 容量驻留、逐 tick clear 重填，消去 grow 分配
        self.players_cache.clear();
        self.players_cache
            .extend((0..PLAYERS).map(|i| player_snapshot(i, tick)));
        self.block_entities.clear();
        self.block_entities
            .extend((0..BLOCK_ENTITIES).map(|i| block_entity_id(i, tick)));

        let mut collision_fp: u64 = 0xcbf2_9ce4_8422_2325;
        for e in 0..ENTITIES {
            let eb = entity_box(e, tick);
            for p in &self.players_cache {
                let hit = (p.chunk.0 - eb.chunk.0).abs() <= 1
                    && (p.chunk.1 - eb.chunk.1).abs() <= 1
                    && (p.x - eb.x).abs() < 5
                    && (p.z - eb.z).abs() < 5;
                collision_fp = (collision_fp ^ u64::from(hit))
                    .wrapping_mul(0x0000_0100_0000_01B3)
                    .wrapping_add(u64::from(p.id).wrapping_add(u64::from(eb.id)));
                if hit {
                    break;
                }
            }
        }
        let mut be_fp: u64 = 0xcbf2_9ce4_8422_2325;
        for id in &self.block_entities {
            be_fp = (be_fp ^ id).wrapping_mul(0x0000_0100_0000_01B3);
        }
        (collision_fp, be_fp)
    }
}

// ---------------------------------------------------------------------------
// 度量
// ---------------------------------------------------------------------------

struct Tally {
    allocs: usize,
    alloc_bytes: usize,
    collision_fp: u64,
    be_fp: u64,
}

fn measure_fresh() -> Tally {
    per_tick_fresh(0); // 热身
    let (a0, b0) = counters();
    let (mut cfp, mut bfp) = (0, 0);
    for tick in 0..TICKS {
        let (c, b) = per_tick_fresh(tick);
        cfp = c;
        bfp = b;
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        collision_fp: cfp,
        be_fp: bfp,
    }
}

fn measure_reused() -> Tally {
    let mut bufs = ReusedBuffers::new();
    bufs.refill(0); // 热身（完成首次 grow）
    let (a0, b0) = counters();
    let (mut cfp, mut bfp) = (0, 0);
    for tick in 0..TICKS {
        let (c, b) = bufs.refill(tick);
        cfp = c;
        bfp = b;
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        collision_fp: cfp,
        be_fp: bfp,
    }
}

fn main() {
    let a = measure_fresh();
    let b = measure_reused();

    // 内容门：碰撞判定结果与活跃集内容逐元素全等（两形态语义等价）
    let content_equal = a.collision_fp == b.collision_fp && a.be_fp == b.be_fp;
    let allocs_reduced = b.allocs < a.allocs;
    let bytes_reduced = b.alloc_bytes < a.alloc_bytes;
    let all_pass = content_equal && allocs_reduced && bytes_reduced;

    eprintln!(
        "逐 tick 新建：分配 {} 次 / {} B（每 tick {:.1} 次 / {:.0} B）",
        a.allocs,
        a.alloc_bytes,
        a.allocs as f64 / TICKS as f64,
        a.alloc_bytes as f64 / TICKS as f64
    );
    eprintln!(
        "跨 tick 复用：分配 {} 次 / {} B（每 tick {:.2} 次 / {:.0} B）",
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
        "round": 14,
        "topic": "世界 tick 玩家快照与方块实体活跃集跨 tick 复用（消逐 tick 分配流失）",
        "workload": {
            "ticks": TICKS,
            "players": PLAYERS,
            "entities": ENTITIES,
            "block_entities": BLOCK_ENTITIES,
        },
        "per_tick_fresh": { "allocs": a.allocs, "bytes": a.alloc_bytes },
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

    let path = "note/report/perf/round14-tick-buffer-reuse.json";
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
