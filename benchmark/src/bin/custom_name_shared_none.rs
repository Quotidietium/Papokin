//! 轮次 47 基准：实体 `custom_name` 空盒共享单例。
//!
//! 对照形态（每实体构造的残余堆分配点）：
//! - 旧态：`custom_name: ArcSwap<Option<TextComponent>>` 构造时
//!   `Arc::new(None)`（每实体一个仅装 `None` 的堆盒，
//!   `Option<TextComponent>` 约 48B，加 malloc 头与规格取整后
//!   每实体约 64B）；绝大多数实体从未命名，纯为空态付费；
//! - 新态：`shared_none_name_arc()` 进程级 `LazyLock` 共享单例
//!   （构造零分配，仅首次访问付一个盒），命名写路径不变
//!   （`set_custom_name` 仍走独立 `Arc::new(Some(..))`）。
//!
//! 负载：20000 实体构造 + 20 轮未命名读电池（`load`+`is_some`）
//! + 10% 实体命名写 + 5 轮混合读电池（命名实体折叠文本字节）。
//!
//! 语义指纹：读电池与命名回读的折叠值全等（双臂共用脚本）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use arc_swap::ArcSwap;
use papokin_util::text::TextComponent;

const ENTITY_COUNT: usize = 20000;
const READ_ROUNDS: usize = 20;
const NAMED_ROUNDS: usize = 5;

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

fn allocs() -> usize {
    ALLOCS.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// 形态复刻（与生产同构的字段与构造点）
// ---------------------------------------------------------------------------

/// 与生产 `Entity.custom_name` 同构的字段形态。
struct Entity {
    custom_name: ArcSwap<Option<TextComponent>>,
}

/// 旧态构造：每实体一个仅装 `None` 的堆盒。
#[allow(clippy::arc_with_non_send_sync)]
fn build_old() -> Vec<Entity> {
    (0..ENTITY_COUNT)
        .map(|_| Entity {
            custom_name: ArcSwap::from(Arc::new(None)),
        })
        .collect()
}

/// 新态构造：空态共享单例（轮次 44 群系盒同法）。
fn shared_none_name_arc() -> Arc<Option<TextComponent>> {
    static NONE_NAME: LazyLock<Arc<Option<TextComponent>>> = LazyLock::new(|| Arc::new(None));
    NONE_NAME.clone()
}

fn build_new() -> Vec<Entity> {
    (0..ENTITY_COUNT)
        .map(|_| Entity {
            custom_name: ArcSwap::from(shared_none_name_arc()),
        })
        .collect()
}

const fn fp_mix(fp: u64, x: u64) -> u64 {
    (fp ^ x).wrapping_mul(0x1000_0000_01b3)
}

// ---------------------------------------------------------------------------
// 双臂（同一电池脚本，仅构造函数不同）
// ---------------------------------------------------------------------------

struct ArmStats {
    construct_allocs: usize,
    construct_ms: f64,
    read_ms: f64,
    naming_allocs: usize,
    resident: usize,
}

/// 未命名读电池：全实体 `load` + `is_some` 折叠（与元数据序列化
/// 的空态检查同路径）。
fn read_battery_unnamed(corpus: &[Entity], fp: &mut u64) {
    for e in corpus {
        *fp = fp_mix(*fp, u64::from(u8::from(e.custom_name.load().is_some())));
    }
}

/// 命名写：10% 实体存独立 `Arc::new(Some(..))`（写路径双臂同码）。
fn name_sample(corpus: &mut [Entity], fp: &mut u64) {
    for (i, e) in corpus.iter_mut().enumerate() {
        if i.is_multiple_of(10) {
            let name = TextComponent::text(format!("实体名牌-{i}"));
            e.custom_name.store(Arc::new(Some(name)));
            *fp = fp_mix(*fp, u64::from(u8::from(e.custom_name.load().is_some())));
        }
    }
}

/// 混合读电池：命名实体回读文本字节（与 `get_name` 的
/// `load`+`as_ref().clone()` 同路径）。
fn read_battery_named(corpus: &[Entity], fp: &mut u64) {
    for e in corpus {
        let guard = e.custom_name.load();
        match guard.as_ref().clone() {
            Some(text) => {
                for b in text.get_text().bytes() {
                    *fp = fp_mix(*fp, u64::from(b));
                }
            }
            None => *fp = fp_mix(*fp, 0x2a),
        }
    }
}

fn run_arm(build: fn() -> Vec<Entity>) -> (u64, ArmStats) {
    let a0 = allocs();
    let start = Instant::now();
    let mut corpus = build();
    let construct_allocs = allocs() - a0;
    let construct_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        read_battery_unnamed(&corpus, &mut fp);
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;

    let a1 = allocs();
    name_sample(&mut corpus, &mut fp);
    let naming_allocs = allocs() - a1;

    for _ in 0..NAMED_ROUNDS {
        read_battery_named(&corpus, &mut fp);
    }

    (
        fp,
        ArmStats {
            construct_allocs,
            construct_ms,
            read_ms,
            naming_allocs,
            resident: resident(),
        },
    )
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn main() {
    println!("轮次 47 基准：实体 custom_name 空盒共享单例");
    println!(
        "语料：{ENTITY_COUNT} 实体 · 未命名读 {READ_ROUNDS} 轮 · 命名 10% · 混合读 {NAMED_ROUNDS} 轮"
    );
    println!();

    let (old_fp, old) = run_arm(build_old);
    let (new_fp, new) = run_arm(build_new);
    let v = evaluate(&old, &new, old_fp, new_fp);
    print_gates(&v, &old, &new, old_fp, new_fp);
    write_report(&v, &old, &new, old_fp, new_fp);
}

struct Verdict {
    semantics_ok: bool,
    resident_ok: bool,
    construct_allocs_ok: bool,
    naming_ok: bool,
    read_ok: bool,
    read_ratio: f64,
    all_passed: bool,
}

fn evaluate(old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let resident_ok = new.resident < old.resident;
    let construct_allocs_ok = new.construct_allocs < old.construct_allocs;
    let naming_ok = new.naming_allocs == old.naming_allocs;
    let read_ratio = old.read_ms / new.read_ms;
    let read_ok = read_ratio >= 0.91;
    Verdict {
        semantics_ok,
        resident_ok,
        construct_allocs_ok,
        naming_ok,
        read_ok,
        read_ratio,
        all_passed: semantics_ok && resident_ok && construct_allocs_ok && naming_ok && read_ok,
    }
}

fn print_gates(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    println!("=== 闸门 ===");
    println!(
        "{} 读写电池指纹全等：旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}",
        mark(v.semantics_ok)
    );
    println!(
        "{} 驻留字节下降（主判据）：{} → {} B（每实体 {:.1} → {:.1} B）",
        mark(v.resident_ok),
        old.resident,
        new.resident,
        old.resident as f64 / ENTITY_COUNT as f64,
        new.resident as f64 / ENTITY_COUNT as f64
    );
    println!(
        "{} 构造分配次数下降：{} → {} 次（每实体 {:.3} → {:.3}）",
        mark(v.construct_allocs_ok),
        old.construct_allocs,
        new.construct_allocs,
        old.construct_allocs as f64 / ENTITY_COUNT as f64,
        new.construct_allocs as f64 / ENTITY_COUNT as f64
    );
    println!(
        "{} 命名写路径不回归（等分配）：{} → {} 次",
        mark(v.naming_ok),
        old.naming_allocs,
        new.naming_allocs
    );
    println!(
        "{} 读耗时不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.read_ok),
        old.read_ms,
        new.read_ms,
        v.read_ratio
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构造：{:.2} → {:.2} ms（×{:.2}）",
        old.construct_ms,
        new.construct_ms,
        old.construct_ms / new.construct_ms
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

fn write_report(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    use std::io::Write as _;

    let report = serde_json::json!({
        "round": 47,
        "topic": "custom_name_shared_none",
        "corpus": {
            "entities": ENTITY_COUNT,
            "read_rounds": READ_ROUNDS,
            "named_rounds": NAMED_ROUNDS,
            "named_ratio": "10%",
            "wrapper_points": ["custom_name_none_arc"],
        },
        "gates": [
            {"name": "读写电池指纹全等", "passed": v.semantics_ok,
             "detail": format!("0x{old_fp:016x} vs 0x{new_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", old.resident, new.resident)},
            {"name": "构造分配次数下降", "passed": v.construct_allocs_ok,
             "detail": format!("{} → {}", old.construct_allocs, new.construct_allocs)},
            {"name": "命名写路径不回归（等分配）", "passed": v.naming_ok,
             "detail": format!("{} → {}", old.naming_allocs, new.naming_allocs)},
            {"name": "读耗时不劣化（≥0.91×）", "passed": v.read_ok,
             "detail": format!("×{:.3}", v.read_ratio)},
        ],
        "old": {
            "resident_bytes": old.resident,
            "construct_allocs": old.construct_allocs,
            "construct_ms": old.construct_ms,
            "read_ms": old.read_ms,
            "naming_allocs": old.naming_allocs,
            "content_fingerprint": format!("0x{old_fp:016x}"),
        },
        "new": {
            "resident_bytes": new.resident,
            "construct_allocs": new.construct_allocs,
            "construct_ms": new.construct_ms,
            "read_ms": new.read_ms,
            "naming_allocs": new.naming_allocs,
            "content_fingerprint": format!("0x{new_fp:016x}"),
        },
        "sizes": {
            "option_textcomponent_bytes": std::mem::size_of::<Option<TextComponent>>(),
            "arc_option_textcomponent_bytes": std::mem::size_of::<Arc<Option<TextComponent>>>(),
            "arcswap_field_bytes": std::mem::size_of::<ArcSwap<Option<TextComponent>>>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round47-custom-name-shared-none.json");
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
