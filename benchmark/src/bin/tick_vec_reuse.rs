//! 轮次 15 基准：世界 tick 热路径剩余六处逐 tick 向量分配流失的量化与收编对照。
//!
//! ## 侦察结论（生产可达性已验证）
//!
//! 轮次 14 收编玩家快照与方块实体活跃集后，`World::tick` 每 tick 仍无条件
//! （或常规配置下等价于无条件）执行以下临时向量分配：
//!
//! 1. **可 tick 实体过滤集**（`world/mod.rs` `tickable`，约行 1423）：对全量
//!    实体按「位于活跃且已加载区块」过滤后 `collect()`，规模 = 实体数 E。
//! 2. **生成候选区块集**（`spawning_chunks`，约行 1932）：`spawn_list` 非空
//!    （默认游戏规则下每 tick 成立）即按活跃区块收集 `(pos, Arc<ChunkData>)`，
//!    规模 = 活跃区块数 A。
//! 3. **活跃区块坐标集**（`active_chunks_vec`，约行 1967）：为 `inhabited_time`
//!    自增把 `FxHashSet` 拷贝成 Vec，规模 = A，无条件执行。
//! 4. **计划刻/随机刻数据**（`level.rs` `get_tick_data` 返回的 `TickData`，
//!    约行 588）：每 tick 新建三个 Vec，其中 `random_ticks` 预分配 A×3，
//!    无条件执行。
//!
//! 六处均为**逐 tick 重建、tick 结束即弃**的 Vec。收编形态与轮次 14 相同：
//! 跨 tick 复用缓冲（容量驻留、逐 tick `clear()` 重填）。本基准量化两形态
//! 在同一负载下的分配账，并以内容指纹作语义等价硬闸门。
//!
//! 硬闸门：两形态产出（四处内容指纹）逐元素全等；复用形态稳态分配次数与
//! 字节严格小于逐 tick 新建。
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
/// 实体数（可 tick 实体过滤集规模）
const ENTITIES: usize = 800;
/// 活跃区块数（生成候选/活跃坐标集/随机刻采样基数）
const ACTIVE_CHUNKS: usize = 600;
/// 每 tick 到期方块计划刻数（稳态小量）
const SCHEDULED_BLOCK: usize = 64;
/// 每 tick 到期流体计划刻数（稳态小量）
const SCHEDULED_FLUID: usize = 32;

/// 随机刻采样数 = 活跃区块 × 3（`random_tick_speed` 默认 3）
const fn random_samples() -> usize {
    ACTIVE_CHUNKS * 3
}

// ---------------------------------------------------------------------------
// 负载模型：四类逐 tick 向量的元素占位（尺寸对齐生产元素）
// ---------------------------------------------------------------------------

/// 实体过滤项：`(Arc 指针对, 区块坐标)`，16 + 8 字节
#[derive(Clone, Copy)]
struct EntityItem {
    id: u64,
    chunk: (i32, i32),
}

/// 生成候选项：`(区块坐标, Arc 指针对)`，8 + 16 字节
#[derive(Clone, Copy)]
struct SpawnItem {
    chunk: (i32, i32),
    token: u64,
}

/// 计划刻项：`OrderedTick` 占位（位置 + 目标 + 优先级等），32 字节
#[derive(Clone, Copy)]
struct TickItem {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
}

/// 随机刻采样：`BlockPos + 两个布尔`，16 字节
#[derive(Clone, Copy)]
struct SampleItem {
    pos: (i32, i32, i32),
    flags: u8,
}

const fn entity_item(i: usize, tick: usize) -> EntityItem {
    let x = (i as i32 * 53 + tick as i32 * 7) % 10000;
    let z = (i as i32 * 71 + tick as i32 * 11) % 10000;
    EntityItem {
        id: (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        chunk: (x >> 4, z >> 4),
    }
}

const fn spawn_item(i: usize, tick: usize) -> SpawnItem {
    SpawnItem {
        chunk: ((i as i32 * 31) % 512, (i as i32 * 17 + tick as i32) % 512),
        token: (i as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
    }
}

const fn chunk_pos(i: usize, tick: usize) -> (i32, i32) {
    ((i as i32 * 31 + tick as i32) % 512, (i as i32 * 17) % 512)
}

const fn tick_item(i: usize, tick: usize, salt: u64) -> TickItem {
    let base = (i as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt)
        .wrapping_add(tick as u64);
    TickItem {
        a: base,
        b: base.rotate_left(17),
        c: base.rotate_left(33),
        d: base.rotate_left(49),
    }
}

const fn sample_item(i: usize, tick: usize) -> SampleItem {
    SampleItem {
        pos: (
            (i as i32 * 13 + tick as i32) % 8192,
            ((i as i32 * 7) % 384) - 64,
            (i as i32 * 29) % 8192,
        ),
        flags: (i % 4) as u8,
    }
}

const fn fp_mix(fp: u64, v: u64) -> u64 {
    (fp ^ v).wrapping_mul(0x0000_0100_0000_01B3)
}

// ---------------------------------------------------------------------------
// 形态 A：逐 tick 新建（生产现状）
// ---------------------------------------------------------------------------

struct Fingerprints {
    entity: u64,
    spawn: u64,
    chunk: u64,
    tick_data: u64,
}

fn per_tick_fresh(tick: usize) -> Fingerprints {
    // 1. 可 tick 实体过滤集：每 tick 新建 Vec
    let tickable: Vec<EntityItem> = (0..ENTITIES).map(|i| entity_item(i, tick)).collect();
    // 2. 生成候选区块集：每 tick 新建 Vec
    let mut spawning: Vec<SpawnItem> = Vec::new();
    for i in 0..ACTIVE_CHUNKS {
        spawning.push(spawn_item(i, tick));
    }
    // 3. 活跃区块坐标集：每 tick 从集合拷贝新建 Vec
    let active_vec: Vec<(i32, i32)> = (0..ACTIVE_CHUNKS).map(|i| chunk_pos(i, tick)).collect();
    // 4. TickData 三 Vec（random_ticks 预分配 A×3）
    let mut block_ticks: Vec<TickItem> = Vec::new();
    block_ticks.extend((0..SCHEDULED_BLOCK).map(|i| tick_item(i, tick, 0x11)));
    let mut fluid_ticks: Vec<TickItem> = Vec::new();
    fluid_ticks.extend((0..SCHEDULED_FLUID).map(|i| tick_item(i, tick, 0x77)));
    let mut random_ticks: Vec<SampleItem> = Vec::with_capacity(random_samples());
    random_ticks.extend((0..random_samples()).map(|i| sample_item(i, tick)));

    let mut entity_fp = 0xcbf2_9ce4_8422_2325u64;
    for item in &tickable {
        entity_fp = fp_mix(entity_fp, item.id ^ (item.chunk.0 as u64) ^ (item.chunk.1 as u64));
    }
    let mut spawn_fp = 0xcbf2_9ce4_8422_2325u64;
    for item in &spawning {
        spawn_fp = fp_mix(spawn_fp, item.token ^ (item.chunk.0 as u64));
    }
    let mut chunk_fp = 0xcbf2_9ce4_8422_2325u64;
    for pos in &active_vec {
        chunk_fp = fp_mix(chunk_fp, (pos.0 as u64) ^ (pos.1 as u64));
    }
    let mut tick_data_fp = 0xcbf2_9ce4_8422_2325u64;
    for item in &block_ticks {
        tick_data_fp = fp_mix(tick_data_fp, item.a ^ item.c);
    }
    for item in &fluid_ticks {
        tick_data_fp = fp_mix(tick_data_fp, item.b ^ item.d);
    }
    for item in &random_ticks {
        tick_data_fp = fp_mix(
            tick_data_fp,
            (item.pos.0 as u64)
                ^ (item.pos.1 as u64)
                ^ (item.pos.2 as u64)
                ^ u64::from(item.flags),
        );
    }
    Fingerprints {
        entity: entity_fp,
        spawn: spawn_fp,
        chunk: chunk_fp,
        tick_data: tick_data_fp,
    }
}

// ---------------------------------------------------------------------------
// 形态 B：跨 tick 复用缓冲（本轮收编形态）
// ---------------------------------------------------------------------------

struct ReusedVecs {
    tickable: Vec<EntityItem>,
    spawning: Vec<SpawnItem>,
    active_vec: Vec<(i32, i32)>,
    block_ticks: Vec<TickItem>,
    fluid_ticks: Vec<TickItem>,
    random_ticks: Vec<SampleItem>,
}

impl ReusedVecs {
    fn new() -> Self {
        Self {
            tickable: Vec::with_capacity(ENTITIES),
            spawning: Vec::with_capacity(ACTIVE_CHUNKS),
            active_vec: Vec::with_capacity(ACTIVE_CHUNKS),
            block_ticks: Vec::with_capacity(SCHEDULED_BLOCK),
            fluid_ticks: Vec::with_capacity(SCHEDULED_FLUID),
            random_ticks: Vec::with_capacity(random_samples()),
        }
    }

    fn refill(&mut self, tick: usize) -> Fingerprints {
        self.tickable.clear();
        self.tickable.extend((0..ENTITIES).map(|i| entity_item(i, tick)));
        self.spawning.clear();
        self.spawning.extend((0..ACTIVE_CHUNKS).map(|i| spawn_item(i, tick)));
        self.active_vec.clear();
        self.active_vec
            .extend((0..ACTIVE_CHUNKS).map(|i| chunk_pos(i, tick)));
        self.block_ticks.clear();
        self.block_ticks
            .extend((0..SCHEDULED_BLOCK).map(|i| tick_item(i, tick, 0x11)));
        self.fluid_ticks.clear();
        self.fluid_ticks
            .extend((0..SCHEDULED_FLUID).map(|i| tick_item(i, tick, 0x77)));
        self.random_ticks.clear();
        self.random_ticks
            .extend((0..random_samples()).map(|i| sample_item(i, tick)));

        let mut entity_fp = 0xcbf2_9ce4_8422_2325u64;
        for item in &self.tickable {
            entity_fp = fp_mix(entity_fp, item.id ^ (item.chunk.0 as u64) ^ (item.chunk.1 as u64));
        }
        let mut spawn_fp = 0xcbf2_9ce4_8422_2325u64;
        for item in &self.spawning {
            spawn_fp = fp_mix(spawn_fp, item.token ^ (item.chunk.0 as u64));
        }
        let mut chunk_fp = 0xcbf2_9ce4_8422_2325u64;
        for pos in &self.active_vec {
            chunk_fp = fp_mix(chunk_fp, (pos.0 as u64) ^ (pos.1 as u64));
        }
        let mut tick_data_fp = 0xcbf2_9ce4_8422_2325u64;
        for item in &self.block_ticks {
            tick_data_fp = fp_mix(tick_data_fp, item.a ^ item.c);
        }
        for item in &self.fluid_ticks {
            tick_data_fp = fp_mix(tick_data_fp, item.b ^ item.d);
        }
        for item in &self.random_ticks {
            tick_data_fp = fp_mix(
                tick_data_fp,
                (item.pos.0 as u64)
                    ^ (item.pos.1 as u64)
                    ^ (item.pos.2 as u64)
                    ^ u64::from(item.flags),
            );
        }
        Fingerprints {
            entity: entity_fp,
            spawn: spawn_fp,
            chunk: chunk_fp,
            tick_data: tick_data_fp,
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
        fps,
    }
}

fn measure_reused() -> Tally {
    let mut bufs = ReusedVecs::new();
    bufs.refill(0); // 热身（完成首次 grow）
    let (a0, b0) = counters();
    let mut fps = bufs.refill(0);
    for tick in 0..TICKS {
        fps = bufs.refill(tick);
    }
    let (a1, b1) = counters();
    Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        fps,
    }
}

fn main() {
    let a = measure_fresh();
    let b = measure_reused();

    // 内容门：四类产出指纹逐元素全等（两形态语义等价）
    let content_equal = a.fps.entity == b.fps.entity
        && a.fps.spawn == b.fps.spawn
        && a.fps.chunk == b.fps.chunk
        && a.fps.tick_data == b.fps.tick_data;
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
        "round": 15,
        "topic": "世界 tick 剩余六处逐 tick 向量跨 tick 复用（实体过滤集/生成候选/活跃坐标/TickData 三 Vec）",
        "workload": {
            "ticks": TICKS,
            "entities": ENTITIES,
            "active_chunks": ACTIVE_CHUNKS,
            "scheduled_block": SCHEDULED_BLOCK,
            "scheduled_fluid": SCHEDULED_FLUID,
            "random_samples": random_samples(),
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

    let path = "note/report/perf/round15-tick-vec-reuse.json";
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
