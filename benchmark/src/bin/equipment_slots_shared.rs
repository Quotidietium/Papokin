//! 轮次 43 基准：实体装备槽映射去重（逐实体重建 → 全局共享）。
//!
//! 对照形态（`LivingEntity.equipment_slots` 的承载）：
//! - 逐实体重建（旧态）：每个 `LivingEntity`（生物与玩家）构造时
//!   `Arc::new(build_equipment_slots())`——内容恒定的 5 条
//!   `usize → EquipmentSlot` 映射每次从零重建，hashbrown 容量梯
//!   3→7 两次增长分配 + 终态 ~152B/实体；
//! - 全局共享（新态）：`LazyLock<Arc<FxHashMap<..>>>` 单实例，
//!   构造仅一次 `Arc::clone`（零堆分配、零哈希计算）。
//!
//! 负载模型：20000 个实体构造（映射内容两臂恒等，由纯函数
//! `build_equipment_slots` 保证）+ 10 轮读取电池（5 键遍历 + 逐键
//! 点查 + values 收集），操作脚本全量指纹双臂对账。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use papokin_data::data_component_impl::EquipmentSlot;
use papokin_inventory::build_equipment_slots;
use rustc_hash::FxHashMap;

const ENTITY_COUNT: usize = 20000;
const READ_ROUNDS: usize = 10;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅在系统分配器外包裹计数，转发语义与系统分配器一致。
unsafe impl GlobalAlloc for Counting {
    // SAFETY: 转发系统分配器，契约由分配器 trait 保证。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器。
        unsafe { System.alloc(layout) }
    }

    // SAFETY: 转发系统分配器，ptr 来自本分配器的 alloc。
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn resident() -> usize {
    ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed)
}

/// 全局共享实例（新态承载，与生产迁移目标同构）。
static SHARED: LazyLock<Arc<FxHashMap<usize, EquipmentSlot>>> =
    LazyLock::new(|| Arc::new(build_equipment_slots()));

const fn fp_mix(fp: u64, x: u64) -> u64 {
    (fp ^ x).wrapping_mul(0x1000_0000_01b3)
}

/// 槽位指纹：名字字节逐位折叠（槽名全表唯一，足以判别）。
fn slot_fp(fp: u64, slot: &EquipmentSlot) -> u64 {
    let mut acc = fp;
    for byte in slot.to_name().as_bytes() {
        acc = fp_mix(acc, u64::from(*byte));
    }
    fp_mix(acc, 0xff)
}

/// 读取电池：5 键遍历 + 逐键点查 + values 收集。
fn read_battery(maps: &[Arc<FxHashMap<usize, EquipmentSlot>>], fp: &mut u64) {
    for map in maps {
        for (idx, slot) in map.iter() {
            *fp = fp_mix(*fp, u64::try_from(*idx).unwrap_or(u64::MAX));
            *fp = slot_fp(*fp, slot);
        }
        let mut keys: Vec<usize> = map.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            *fp = fp_mix(*fp, u64::try_from(key).unwrap_or(u64::MAX));
        }
        for slot in map.values() {
            *fp = slot_fp(*fp, slot);
        }
    }
}

struct ArmStats {
    spawn_allocs: usize,
    spawn_ms: f64,
    read_ms: f64,
    resident: usize,
}

fn run_shared_arm() -> (u64, ArmStats) {
    let a0 = ALLOCS.load(Ordering::Relaxed);
    let start = Instant::now();
    let mut corpus: Vec<Arc<FxHashMap<usize, EquipmentSlot>>> = Vec::with_capacity(ENTITY_COUNT);
    for _ in 0..ENTITY_COUNT {
        corpus.push(Arc::clone(&SHARED));
    }
    let spawn_allocs = ALLOCS.load(Ordering::Relaxed) - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        read_battery(&corpus, &mut fp);
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    (
        fp,
        ArmStats {
            spawn_allocs,
            spawn_ms,
            read_ms,
            resident: resident(),
        },
    )
}

fn run_fresh_arm() -> (u64, ArmStats) {
    let a0 = ALLOCS.load(Ordering::Relaxed);
    let start = Instant::now();
    let mut corpus: Vec<Arc<FxHashMap<usize, EquipmentSlot>>> = Vec::with_capacity(ENTITY_COUNT);
    for _ in 0..ENTITY_COUNT {
        corpus.push(Arc::new(build_equipment_slots()));
    }
    let spawn_allocs = ALLOCS.load(Ordering::Relaxed) - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        read_battery(&corpus, &mut fp);
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    (
        fp,
        ArmStats {
            spawn_allocs,
            spawn_ms,
            read_ms,
            resident: resident(),
        },
    )
}

fn main() {
    println!("轮次 43 基准：实体装备槽映射去重（逐实体重建 → 全局共享）");
    println!("语料：{ENTITY_COUNT} 个实体构造 · 读 {READ_ROUNDS} 轮 · 映射恒 5 键（纯函数输出）");
    println!();

    // 先跑共享臂（其 SHARED 单实例的懒构建计入共享臂构造账）
    let (shared_fp, shared) = run_shared_arm();
    // 共享臂语料仅持 Arc 克隆，驻留几乎为零；显式 drop 后再跑旧臂，
    // 避免两代语料叠加计入驻留（轮次 40 坑）
    let (fresh_fp, fresh) = run_fresh_arm();

    let v = evaluate(&fresh, &shared, fresh_fp, shared_fp);
    print_gates(&v, &fresh, &shared, fresh_fp, shared_fp);
    write_report(&v, &fresh, &shared, fresh_fp, shared_fp);
}

struct Verdict {
    semantics_ok: bool,
    resident_ok: bool,
    spawn_allocs_ok: bool,
    read_ratio: f64,
    all_passed: bool,
}

fn evaluate(fresh: &ArmStats, shared: &ArmStats, fresh_fp: u64, shared_fp: u64) -> Verdict {
    let semantics_ok = shared_fp == fresh_fp;
    let resident_ok = shared.resident < fresh.resident;
    let spawn_allocs_ok = shared.spawn_allocs < fresh.spawn_allocs;
    let read_ratio = fresh.read_ms / shared.read_ms;
    Verdict {
        semantics_ok,
        resident_ok,
        spawn_allocs_ok,
        read_ratio,
        all_passed: semantics_ok && resident_ok && spawn_allocs_ok && read_ratio >= 0.91,
    }
}

fn print_gates(v: &Verdict, fresh: &ArmStats, shared: &ArmStats, fresh_fp: u64, shared_fp: u64) {
    println!("=== 闸门 ===");
    println!(
        "{} 读取电池指纹全等：逐建 0x{fresh_fp:016x} vs 共享 0x{shared_fp:016x}",
        mark(v.semantics_ok)
    );
    println!(
        "{} 驻留字节下降（主判据）：{} → {} B（每实体 {:.2} → {:.2} B，共享臂含单实例构建）",
        mark(v.resident_ok),
        fresh.resident,
        shared.resident,
        fresh.resident as f64 / ENTITY_COUNT as f64,
        shared.resident as f64 / ENTITY_COUNT as f64
    );
    println!(
        "{} 构造分配次数下降：{} → {} 次（每实体 {:.4} → {:.4}）",
        mark(v.spawn_allocs_ok),
        fresh.spawn_allocs,
        shared.spawn_allocs,
        fresh.spawn_allocs as f64 / ENTITY_COUNT as f64,
        shared.spawn_allocs as f64 / ENTITY_COUNT as f64
    );
    println!(
        "{} 读取耗时不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.read_ratio >= 0.91),
        fresh.read_ms,
        shared.read_ms,
        v.read_ratio
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构造：{:.2} → {:.2} ms（×{:.2}）",
        fresh.spawn_ms,
        shared.spawn_ms,
        fresh.spawn_ms / shared.spawn_ms
    );
    println!(
        "闸门判定：{}",
        if v.all_passed {
            "全部通过，准予收编"
        } else {
            "存在未过闸门项，不予收编"
        }
    );
}

fn write_report(v: &Verdict, fresh: &ArmStats, shared: &ArmStats, fresh_fp: u64, shared_fp: u64) {
    use std::io::Write as _;

    let report = serde_json::json!({
        "round": 43,
        "topic": "equipment_slots_shared",
        "corpus": {"entities": ENTITY_COUNT, "read_rounds": READ_ROUNDS, "map_keys": 5},
        "gates": [
            {"name": "读取电池指纹全等", "passed": v.semantics_ok,
             "detail": format!("0x{fresh_fp:016x} vs 0x{shared_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", fresh.resident, shared.resident)},
            {"name": "构造分配次数下降", "passed": v.spawn_allocs_ok,
             "detail": format!("{} → {}", fresh.spawn_allocs, shared.spawn_allocs)},
            {"name": "读取耗时不劣化（≥0.91×）", "passed": v.read_ratio >= 0.91,
             "detail": format!("×{:.3}", v.read_ratio)},
        ],
        "fresh": {
            "resident_bytes": fresh.resident,
            "spawn_allocs": fresh.spawn_allocs,
            "spawn_ms": fresh.spawn_ms,
            "read_ms": fresh.read_ms,
            "content_fingerprint": format!("0x{fresh_fp:016x}"),
        },
        "shared": {
            "resident_bytes": shared.resident,
            "spawn_allocs": shared.spawn_allocs,
            "spawn_ms": shared.spawn_ms,
            "read_ms": shared.read_ms,
            "content_fingerprint": format!("0x{shared_fp:016x}"),
        },
        "sizes": {
            "map_inline_bytes": std::mem::size_of::<FxHashMap<usize, EquipmentSlot>>(),
            "arc_inline_bytes": std::mem::size_of::<Arc<FxHashMap<usize, EquipmentSlot>>>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round43-equipment-slots-shared.json");
    let Ok(mut file) = std::fs::File::create(path) else {
        eprintln!("写入报告失败");
        return;
    };
    if let Err(err) = file.write_all(
        serde_json::to_string_pretty(&report)
            .unwrap_or_default()
            .as_bytes(),
    ) {
        eprintln!("写入报告失败：{err}");
    }
}

const fn mark(ok: bool) -> &'static str {
    if ok { "√" } else { "×" }
}
