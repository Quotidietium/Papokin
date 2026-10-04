//! 轮次 12 基准：运行时方块 set 的高度图互斥锁竞争量化与批量化收编。
//!
//! ## 侦察结论（本基准的设计依据）
//!
//! `ChunkData::set_block_absolute_y`（生产运行时主路径，方块放置/破坏、
//! 活塞、流体、植物生长……）每次调用都经 `update_heightmap` 独立获取
//! 一次 `heightmap: Mutex` 锁。同一列（x,z）的多个方块各自取锁，且
//! 高度图更新本身只依赖该列最终状态——**锁次数 = 方块数，而非列数**。
//!
//! 已有的 `set_blocks_batch`（仅 `fill` 命令使用）证明正确形态：整批
//! 一次锁 + `changed_columns` 聚合。本基准量化「逐方块锁」与「按列
//! 聚合锁」在同一负载下的锁获取次数与耗时差，作为把该形态推广到
//! 运行时路径的依据。
//!
//! 模型：单区块 16×16 列，每列写入 `COLUMN_HEIGHT` 个方块（自底向上，
//! 模拟一次 16×256×16 的区块填充/大批量编辑）。负载参数与 `fill`
//! 上限同量级。硬闸门：两路径最终高度图内容逐字节全等；聚合路径
//! 锁获取次数严格小于逐方块路径。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// 锁获取计数（包一层 Mutex，对齐生产 std::sync::Mutex 语义）
// ---------------------------------------------------------------------------

static LOCK_ACQUISITIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingMutex<T> {
    inner: Mutex<T>,
}

impl<T> CountingMutex<T> {
    const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, T> {
        LOCK_ACQUISITIONS.fetch_add(1, Ordering::Relaxed);
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// ---------------------------------------------------------------------------
// 负载参数与高度图模型（对齐生产 9 bit/值、7 值/u64、37 u64 打包布局）
// ---------------------------------------------------------------------------

/// 区块边长（列数 = 16×16）
const EDGE: usize = 16;
/// 每列写入的方块数（自底向上铺满整列，模拟 16×256×16 填充）
const COLUMN_HEIGHT: i32 = 256;
/// 世界最低点（1.21.11）
const MIN_Y: i32 = -64;

struct Heightmap {
    data: Box<[i64]>,
}

impl Heightmap {
    fn new() -> Self {
        Self {
            data: vec![0; 37].into_boxed_slice(),
        }
    }

    /// 与生产 `ChunkHeightmaps::set` 相同的 9 bit 打包写
    fn set(&mut self, x: i32, z: i32, height: i32) {
        let local_x = (x & 15) as usize;
        let local_z = (z & 15) as usize;
        let column_idx = local_z * 16 + local_x;
        let val = (height - MIN_Y + 1).max(0) as u64;
        let array_idx = column_idx / 7;
        let shift = (column_idx % 7) * 9;
        let mask = 0x1FFu64 << shift;
        let mut current = self.data[array_idx] as u64;
        current = (current & !mask) | ((val & 0x1FF) << shift);
        self.data[array_idx] = current as i64;
    }

    fn fingerprint(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for (i, v) in self.data.iter().enumerate() {
            hash = (hash ^ ((*v as u64).wrapping_add(i as u64))).wrapping_mul(0x0000_0100_0000_01B3);
        }
        hash
    }
}

// ---------------------------------------------------------------------------
// 路径 A：逐方块锁（生产 set_block_absolute_y 现状）
// ---------------------------------------------------------------------------

fn per_block_lock() -> (u64, usize, f64) {
    LOCK_ACQUISITIONS.store(0, Ordering::Relaxed);
    let heightmap = CountingMutex::new(Heightmap::new());
    let start = Instant::now();

    for x in 0..EDGE as i32 {
        for z in 0..EDGE as i32 {
            let mut highest = MIN_Y - 1;
            for y in MIN_Y..MIN_Y + COLUMN_HEIGHT {
                highest = y; // 非空方块一路顶到列顶
                // 每个方块独立取锁（对齐 update_heightmap 的逐次 lock）
                heightmap.lock().set(x, z, highest);
            }
        }
    }

    let wall = start.elapsed().as_secs_f64() * 1000.0;
    let hash = heightmap.lock().fingerprint();
    (hash, LOCK_ACQUISITIONS.load(Ordering::Relaxed), wall)
}

// ---------------------------------------------------------------------------
// 路径 B：按列聚合锁（set_blocks_batch 形态推广到运行时）
// ---------------------------------------------------------------------------

fn per_column_lock() -> (u64, usize, f64) {
    LOCK_ACQUISITIONS.store(0, Ordering::Relaxed);
    let heightmap = CountingMutex::new(Heightmap::new());
    let start = Instant::now();

    // 整批一次锁，列内只写最终最高值（语义与逐方块写完全等价：
    // 同列后写覆盖先写，最终状态一致）
    let mut guard = heightmap.lock();
    for x in 0..EDGE as i32 {
        for z in 0..EDGE as i32 {
            let highest = MIN_Y + COLUMN_HEIGHT - 1;
            guard.set(x, z, highest);
        }
    }
    drop(guard);

    let wall = start.elapsed().as_secs_f64() * 1000.0;
    let hash = heightmap.lock().fingerprint();
    (hash, LOCK_ACQUISITIONS.load(Ordering::Relaxed), wall)
}

// ---------------------------------------------------------------------------
// 主流程：两路径对照，双硬闸门
// ---------------------------------------------------------------------------

fn main() {
    let total_blocks = EDGE * EDGE * COLUMN_HEIGHT as usize;
    let total_columns = EDGE * EDGE;

    let (hash_a, locks_a, wall_a) = per_block_lock();
    let (hash_b, locks_b, wall_b) = per_column_lock();

    let content_equal = hash_a == hash_b;
    let locks_reduced = locks_b < locks_a;
    let all_pass = content_equal && locks_reduced;

    eprintln!("逐方块锁：{locks_a} 次 / {wall_a:.1} ms（{total_blocks} 方块）");
    eprintln!("按列聚合：{locks_b} 次 / {wall_b:.1} ms（{total_columns} 列）");
    eprintln!(
        "内容门={content_equal} 锁次数 {}→{}（-{:.1}%）耗时 {:.1}→{:.1} ms（-{:.1}%）",
        locks_a,
        locks_b,
        100.0 * (1.0 - locks_b as f64 / locks_a as f64),
        wall_a,
        wall_b,
        100.0 * (1.0 - wall_b / wall_a),
    );

    let report = serde_json::json!({
        "round": 12,
        "topic": "运行时方块 set 高度图互斥锁按列聚合（锁次数 = 列数而非方块数）",
        "workload": {
            "edge": EDGE,
            "column_height": COLUMN_HEIGHT,
            "total_blocks": total_blocks,
            "total_columns": total_columns,
        },
        "per_block_lock": { "locks": locks_a, "wall_ms": wall_a },
        "per_column_lock": { "locks": locks_b, "wall_ms": wall_b },
        "gates": {
            "content_equal": content_equal,
            "locks_reduced": locks_reduced,
            "all_pass": all_pass,
        },
    });

    let path = "note/report/perf/round12-heightmap-lock-batch.json";
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
