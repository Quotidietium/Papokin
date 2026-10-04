//! 轮次 16 基准：实体移动碰撞收集 / 实体追踪器快照 / 生成列表三处
//! 逐 tick 分配流失的量化与收编对照。
//!
//! ## 侦察结论（生产可达性已验证）
//!
//! 1. **实体移动碰撞收集**（`world/mod.rs` `get_block_collisions`）：每次
//!    调用返回两个新建 `Vec`（碰撞形状 + 方块位置），五个调用点——实体
//!    移动 `adjust_movement_for_collisions`（每个移动实体每 tick）与四个
//!    弹射物路径（箭/三叉戟/浮漂/弹射物基类，每个飞行弹射物每 tick）。
//!    站立/行走实体每 tick 与地面碰撞，两个 Vec 均非空、必分配。
//!    规模 = 移动实体数 M×2 次/tick。
//! 2. **实体追踪器快照**（`entity_tracker.rs` `update_all`）：每 tick
//!    `entity_map.iter().map(clone).collect()` 收集全部受追踪实体，
//!    规模 = 受追踪实体数 E。**无条件**执行。
//! 3. **生成列表**（`World::tick` 生成段）：每 tick 新建
//!    `Vec<&'static MobCategory>` 并 `Arc::new` 包装（批次闭包逐批
//!    Arc clone），而 `tick_spawning_chunk` 形参本就是 `&Vec` 借用。
//!    **无条件**执行。
//!
//! 收编形态：① 碰撞收集改线程局部暂存（rayon 工作线程各持一份，
//! 调用点 clear 重填、就地消费）；② 追踪器快照改 `Mutex` 驻留 Vec
//! 重填；③ 生成列表入 `TickScratch`、直传借用去 Arc。本基准量化
//! 两形态分配账，并以内容指纹作语义等价硬闸门。
//!
//! 硬闸门：两形态产出（碰撞内容、快照内容、生成列表内容指纹）逐元素
//! 全等；复用形态稳态分配次数与字节严格小于逐 tick 新建。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::Arc;
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
/// 移动实体数（每 tick 与地面碰撞、两 Vec 均非空）
const MOVING_ENTITIES: usize = 800;
/// 受追踪实体数（追踪器快照规模）
const TRACKED_ENTITIES: usize = 800;
/// 生成类别数（原版 `SPAWNING_CATEGORIES` 量级）
const SPAWN_CATEGORIES: usize = 8;

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 碰撞形状占位：`BoundingBox`（6×f64），48 字节
#[derive(Clone, Copy)]
struct Shape {
    v: [u64; 6],
}

/// 一次移动碰撞收集的结果占位
struct CollisionPair {
    collisions: Vec<Shape>,
    positions: Vec<(usize, u64)>,
}

/// 确定性生成第 e 实体第 tick 的碰撞形状集（1-6 个，站立实体必贴地）
const fn entity_shapes(e: usize, tick: usize) -> (usize, u64) {
    let count = 1 + (e * 7 + tick * 3) % 6;
    let salt = (e as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick as u64);
    (count, salt)
}

const fn shape_at(salt: u64, i: usize) -> Shape {
    let base = salt.wrapping_add(i as u64).rotate_left((i % 63) as u32);
    Shape {
        v: [
            base,
            base.rotate_left(11),
            base.rotate_left(23),
            base.rotate_left(37),
            base.rotate_left(41),
            base.rotate_left(53),
        ],
    }
}

const fn tracked_token(i: usize, tick: usize) -> u64 {
    (i as u64)
        .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        .wrapping_add(tick as u64)
}

const fn fp_mix(fp: u64, v: u64) -> u64 {
    (fp ^ v).wrapping_mul(0x0000_0100_0000_01B3)
}

// ---------------------------------------------------------------------------
// 形态 A：逐 tick/逐调用新建（生产现状）
// ---------------------------------------------------------------------------

struct Fingerprints {
    collision: u64,
    snapshot: u64,
    spawn_list: u64,
}

/// 模拟 `get_block_collisions`：返回两个新建 Vec
fn collect_collisions_fresh(e: usize, tick: usize) -> CollisionPair {
    let (count, salt) = entity_shapes(e, tick);
    let mut collisions = Vec::new();
    let mut positions = Vec::new();
    for i in 0..count {
        collisions.push(shape_at(salt, i));
        positions.push((i, salt.wrapping_add(i as u64)));
    }
    CollisionPair {
        collisions,
        positions,
    }
}

/// 模拟生成列表：新建 Vec + Arc 包装（忠实复刻生产旧形态的分配账）
#[allow(clippy::rc_buffer)] // 刻意建模生产旧的 `Arc<Vec<..>>` 包装
fn spawn_list_fresh(tick: usize) -> Arc<Vec<u64>> {
    let mut list = Vec::with_capacity(SPAWN_CATEGORIES);
    for (i, allowed) in SPAWNING.iter().enumerate() {
        if *allowed && (i + tick) % 5 != 4 {
            list.push(i as u64);
        }
    }
    Arc::new(list)
}

/// 生成类别可用性（确定性，模拟全局容量裁决）
const SPAWNING: [bool; SPAWN_CATEGORIES] = [true, true, true, true, true, true, false, true];

fn per_tick_fresh(tick: usize) -> Fingerprints {
    // 1. 移动实体碰撞收集：每实体两新建 Vec
    let mut collision_fp = 0xcbf2_9ce4_8422_2325u64;
    for e in 0..MOVING_ENTITIES {
        let pair = collect_collisions_fresh(e, tick);
        for shape in &pair.collisions {
            collision_fp = fp_mix(collision_fp, shape.v[0] ^ shape.v[3]);
        }
        for (len, pos) in &pair.positions {
            collision_fp = fp_mix(collision_fp, (*len as u64) ^ *pos);
        }
    }
    // 2. 追踪器快照：每 tick 新建 Vec 收集全部受追踪实体
    let snapshot: Vec<u64> = (0..TRACKED_ENTITIES)
        .map(|i| tracked_token(i, tick))
        .collect();
    let mut snapshot_fp = 0xcbf2_9ce4_8422_2325u64;
    for token in &snapshot {
        snapshot_fp = fp_mix(snapshot_fp, *token);
    }
    // 3. 生成列表：每 tick 新建 Vec + Arc 包装
    let list = spawn_list_fresh(tick);
    let mut spawn_fp = 0xcbf2_9ce4_8422_2325u64;
    for cat in list.iter() {
        spawn_fp = fp_mix(spawn_fp, *cat);
    }
    Fingerprints {
        collision: collision_fp,
        snapshot: snapshot_fp,
        spawn_list: spawn_fp,
    }
}

// ---------------------------------------------------------------------------
// 形态 B：跨 tick 复用缓冲（本轮收编形态）
// ---------------------------------------------------------------------------

struct ReusedScratch {
    collisions: Vec<Shape>,
    positions: Vec<(usize, u64)>,
    snapshot: Vec<u64>,
    spawn_list: Vec<u64>,
}

impl ReusedScratch {
    fn new() -> Self {
        Self {
            collisions: Vec::new(),
            positions: Vec::new(),
            snapshot: Vec::with_capacity(TRACKED_ENTITIES),
            spawn_list: Vec::with_capacity(SPAWN_CATEGORIES),
        }
    }

    /// 模拟线程局部暂存：clear 重填、就地消费
    fn collect_collisions_reused(&mut self, e: usize, tick: usize) -> u64 {
        let (count, salt) = entity_shapes(e, tick);
        self.collisions.clear();
        self.positions.clear();
        for i in 0..count {
            self.collisions.push(shape_at(salt, i));
            self.positions.push((i, salt.wrapping_add(i as u64)));
        }
        let mut fp = 0xcbf2_9ce4_8422_2325u64;
        for shape in &self.collisions {
            fp = fp_mix(fp, shape.v[0] ^ shape.v[3]);
        }
        for (len, pos) in &self.positions {
            fp = fp_mix(fp, (*len as u64) ^ *pos);
        }
        fp
    }

    fn refill(&mut self, tick: usize) -> Fingerprints {
        let mut collision_fp = 0xcbf2_9ce4_8422_2325u64;
        for e in 0..MOVING_ENTITIES {
            let fp = self.collect_collisions_reused(e, tick);
            collision_fp = fp_mix(collision_fp, fp);
        }
        // 注：形态 A 的 collision_fp 混合路径与形态 B 不同（A 逐实体平坦
        // 混合、B 按实体分段再混合）——为保证两形态指纹可比，B 另设
        // 平坦校验指纹。
        self.snapshot.clear();
        self.snapshot
            .extend((0..TRACKED_ENTITIES).map(|i| tracked_token(i, tick)));
        let mut snapshot_fp = 0xcbf2_9ce4_8422_2325u64;
        for token in &self.snapshot {
            snapshot_fp = fp_mix(snapshot_fp, *token);
        }
        self.spawn_list.clear();
        for (i, allowed) in SPAWNING.iter().enumerate() {
            if *allowed && (i + tick) % 5 != 4 {
                self.spawn_list.push(i as u64);
            }
        }
        let mut spawn_fp = 0xcbf2_9ce4_8422_2325u64;
        for cat in &self.spawn_list {
            spawn_fp = fp_mix(spawn_fp, *cat);
        }
        Fingerprints {
            collision: collision_fp,
            snapshot: snapshot_fp,
            spawn_list: spawn_fp,
        }
    }
}

// ---------------------------------------------------------------------------
// 度量
// ---------------------------------------------------------------------------

struct Tally {
    allocs: usize,
    alloc_bytes: usize,
    fps: Fingerprints,
    flat_collision_fp: u64,
}

fn measure_fresh() -> Tally {
    per_tick_fresh(0); // 热身
    let (a0, b0) = counters();
    let mut fps = per_tick_fresh(0);
    for tick in 0..TICKS {
        fps = per_tick_fresh(tick);
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        flat_collision_fp: fps.collision,
        fps,
    }
}

fn measure_reused() -> Tally {
    let mut bufs = ReusedScratch::new();
    bufs.refill(0); // 热身（完成首次 grow）
    let (a0, b0) = counters();
    let mut fps = bufs.refill(0);
    // 平坦校验指纹：与形态 A 同路径逐形状/逐位置混合
    let mut flat = 0xcbf2_9ce4_8422_2325u64;
    for tick in 0..TICKS {
        fps = bufs.refill(tick);
        if tick == TICKS - 1 {
            for e in 0..MOVING_ENTITIES {
                bufs.collect_collisions_reused(e, tick);
                for shape in &bufs.collisions {
                    flat = fp_mix(flat, shape.v[0] ^ shape.v[3]);
                }
                for (len, pos) in &bufs.positions {
                    flat = fp_mix(flat, (*len as u64) ^ *pos);
                }
            }
        }
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        flat_collision_fp: flat,
        fps,
    }
}

fn main() {
    let a = measure_fresh();
    let b = measure_reused();

    // 内容门：碰撞收集（平坦校验）、快照、生成列表指纹逐元素全等
    let content_equal = a.flat_collision_fp == b.flat_collision_fp
        && a.fps.snapshot == b.fps.snapshot
        && a.fps.spawn_list == b.fps.spawn_list;
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
        "round": 16,
        "topic": "实体移动碰撞收集/追踪器快照/生成列表跨 tick 复用（线程局部暂存 + 驻留重填 + 去 Arc）",
        "workload": {
            "ticks": TICKS,
            "moving_entities": MOVING_ENTITIES,
            "tracked_entities": TRACKED_ENTITIES,
            "spawn_categories": SPAWN_CATEGORIES,
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

    let path = "note/report/perf/round16-collision-tracker-scratch.json";
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
