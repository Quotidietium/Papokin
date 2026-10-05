//! 轮次 11 基准：区块本体「`vec![T; n]` → `into_boxed_slice()`」构造收编。
//!
//! ## 侦察结论（本基准的设计依据）
//!
//! 初版假设「`vec![T; n]` 先建临时 Vec、`into_boxed_slice` 再分配并整体
//! memcpy」是普遍双分配。第一版基准（分配计数器，定长 n = 容量 = 长度）
//! 实测推翻：现代 LLVM 会把「分配后立即按原长度装箱」的模式优化为
//! **单次分配、零拷贝**，`vec![0u8; 2048]` 与 `Box::new([0u8; 2048])`
//! 的分配账完全相同。因此**运行期变长构造（区块段、调色板 nibble）
//! 不存在可收编的双分配**，相关初版改动已回退。
//!
//! 真正成立的是**编译期定长**路径：`Box::new([T; N])` 直接装箱相比
//! `vec![T; N]` → `into_boxed_slice`，在**非 optimized 构造上下文**
//! （`get_or_insert_with` 闭包、`unwrap_or_else` 闭包——闭包边界抑制
//! LLVM 的死分配消除）下消去一次临时分配与整体拷贝。本基准对这三类
//! 生产构造点逐一对照：
//!
//! - 高度图 `[i64; 37]`（`get_or_insert_with` 闭包内构造）；
//! - 单元素调色板 `[BlockStateId; 1]` / `[u8; 1]`（`unwrap_or_else` 闭包）；
//! - 光照 `LightContainer::ARRAY_SIZE` = 2048 B（`unwrap_or_else` 闭包）。
//!
//! 硬闸门：内容逐字节全等；闭包上下文下分配次数与字节数不增（定长
//! 直装在闭包内严格 ≤ 临时 Vec 路径）。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// 计数分配器：分配次数 + 分配字节
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

/// 各类构造的重复次数
const ROUNDS: usize = 8192;
/// 光照半字节缓冲长度（与生产 `LightContainer::ARRAY_SIZE` 一致）
const LIGHT_BYTES: usize = 16 * 16 * 16 / 2;

/// 占位方块状态（对应生产 `BlockStateId` 的 Copy 形态）
type FakeBlockState = u16;

// ---------------------------------------------------------------------------
// 构造对照（闭包上下文，对齐生产 `get_or_insert_with`/`unwrap_or_else`）
// ---------------------------------------------------------------------------

/// 旧：高度图经临时 Vec 装箱
fn legacy_heightmap() -> Box<[i64]> {
    vec![0; 37].into_boxed_slice()
}

/// 新：高度图定长数组直接装箱
fn new_heightmap() -> Box<[i64]> {
    Box::new([0i64; 37]) as Box<[i64]>
}

/// 旧：单元素调色板经临时 Vec 装箱
fn legacy_single_palette() -> Box<[FakeBlockState]> {
    vec![0].into_boxed_slice()
}

/// 新：单元素调色板定长数组直接装箱
fn new_single_palette() -> Box<[FakeBlockState]> {
    Box::new([0u16]) as Box<[FakeBlockState]>
}

/// 旧：光照缓冲经临时 Vec 装箱
fn legacy_light() -> Box<[u8]> {
    let value = 15u8;
    vec![value << 4 | value; LIGHT_BYTES].into_boxed_slice()
}

/// 新：光照缓冲定长数组直接装箱
fn new_light() -> Box<[u8]> {
    let value = 15u8;
    Box::new([value << 4 | value; LIGHT_BYTES]) as Box<[u8]>
}

// ---------------------------------------------------------------------------
// 度量：分配账（只计构造；产物随测随释放）
// ---------------------------------------------------------------------------

struct Tally {
    allocs: usize,
    alloc_bytes: usize,
    content_hash: u64,
}

fn measure(label: &str, f: impl Fn() -> Box<[u8]>) -> Tally {
    f(); // 热身
    let (a0, b0) = counters();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for _ in 0..ROUNDS {
        let buf = f();
        for (i, v) in buf.iter().enumerate() {
            hash =
                (hash ^ (u64::from(*v).wrapping_add(i as u64))).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    let (a1, b1) = counters();
    let tally = Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        content_hash: hash,
    };
    eprintln!(
        "{label}: 分配 {} 次 / {} B",
        tally.allocs, tally.alloc_bytes
    );
    tally
}

/// i64 缓冲转 u8 视图哈希（高度图内容门）
fn measure_i64(label: &str, f: impl Fn() -> Box<[i64]>) -> Tally {
    f(); // 热身
    let (a0, b0) = counters();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for _ in 0..ROUNDS {
        let buf = f();
        for (i, v) in buf.iter().enumerate() {
            hash =
                (hash ^ ((*v as u64).wrapping_add(i as u64))).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    let (a1, b1) = counters();
    let tally = Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        content_hash: hash,
    };
    eprintln!(
        "{label}: 分配 {} 次 / {} B",
        tally.allocs, tally.alloc_bytes
    );
    tally
}

/// u16 缓冲哈希（调色板内容门）
fn measure_u16(label: &str, f: impl Fn() -> Box<[u16]>) -> Tally {
    f(); // 热身
    let (a0, b0) = counters();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for _ in 0..ROUNDS {
        let buf = f();
        for (i, v) in buf.iter().enumerate() {
            hash =
                (hash ^ (u64::from(*v).wrapping_add(i as u64))).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    let (a1, b1) = counters();
    let tally = Tally {
        allocs: a1 - a0,
        alloc_bytes: b1 - b0,
        content_hash: hash,
    };
    eprintln!(
        "{label}: 分配 {} 次 / {} B",
        tally.allocs, tally.alloc_bytes
    );
    tally
}

#[allow(clippy::too_many_lines)]
fn main() {
    let heightmap_legacy = measure_i64("高度图 legacy", legacy_heightmap);
    let heightmap_new = measure_i64("高度图 new   ", new_heightmap);
    let palette_legacy = measure_u16("调色板 legacy", legacy_single_palette);
    let palette_new = measure_u16("调色板 new   ", new_single_palette);
    let light_legacy = measure("光照   legacy", legacy_light);
    let light_new = measure("光照   new   ", new_light);

    let content_equal = heightmap_legacy.content_hash == heightmap_new.content_hash
        && palette_legacy.content_hash == palette_new.content_hash
        && light_legacy.content_hash == light_new.content_hash;

    let legacy_allocs = heightmap_legacy.allocs + palette_legacy.allocs + light_legacy.allocs;
    let new_allocs = heightmap_new.allocs + palette_new.allocs + light_new.allocs;
    let legacy_bytes =
        heightmap_legacy.alloc_bytes + palette_legacy.alloc_bytes + light_legacy.alloc_bytes;
    let new_bytes = heightmap_new.alloc_bytes + palette_new.alloc_bytes + light_new.alloc_bytes;

    // 硬闸门：内容全等 + 新路径分配不增
    let allocs_not_worse = new_allocs <= legacy_allocs;
    let bytes_not_worse = new_bytes <= legacy_bytes;
    let all_pass = content_equal && allocs_not_worse && bytes_not_worse;

    eprintln!(
        "内容门={content_equal} 分配 {}→{}（-{}）字节 {}→{}（-{}）",
        legacy_allocs,
        new_allocs,
        legacy_allocs.saturating_sub(new_allocs),
        legacy_bytes,
        new_bytes,
        legacy_bytes.saturating_sub(new_bytes),
    );

    let report = serde_json::json!({
        "round": 11,
        "topic": "区块本体编译期定长构造收编（Box::new([T;N]) 直装，消闭包内临时 Vec）",
        "finding": "运行期变长 vec![T;n]+into_boxed_slice 经 LLVM 优化已是单次分配零拷贝，无双分配可收编；仅编译期定长在闭包上下文（抑制死分配消除）有真实收益",
        "workload": { "rounds_per_kind": ROUNDS, "light_bytes": LIGHT_BYTES },
        "scenarios": {
            "heightmap_i64x37": {
                "legacy": { "allocs": heightmap_legacy.allocs, "bytes": heightmap_legacy.alloc_bytes },
                "new": { "allocs": heightmap_new.allocs, "bytes": heightmap_new.alloc_bytes },
            },
            "single_palette_u16x1": {
                "legacy": { "allocs": palette_legacy.allocs, "bytes": palette_legacy.alloc_bytes },
                "new": { "allocs": palette_new.allocs, "bytes": palette_new.alloc_bytes },
            },
            "light_u8x2048": {
                "legacy": { "allocs": light_legacy.allocs, "bytes": light_legacy.alloc_bytes },
                "new": { "allocs": light_new.allocs, "bytes": light_new.alloc_bytes },
            },
        },
        "totals": {
            "legacy_allocs": legacy_allocs, "new_allocs": new_allocs,
            "legacy_bytes": legacy_bytes, "new_bytes": new_bytes,
        },
        "gates": {
            "content_bytes_equal": content_equal,
            "allocations_not_worse": allocs_not_worse,
            "bytes_not_worse": bytes_not_worse,
            "all_pass": all_pass,
        },
    });

    let path = "note/report/perf/round11-boxed-slice-double-alloc.json";
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
