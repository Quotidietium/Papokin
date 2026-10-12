//! 轮次 48 基准：方块实体与投射物冗余 `Arc` 盒去化。
//!
//! 对照形态（每实例构造的堆盒，全仓审计确认零克隆点）：
//! - 旧态：告示牌 `SignText.messages/filtered_messages:
//!   Arc<Mutex<[Box<str>; 4]>>`（每面 2 盒 × 双面 = 4 盒/牌）与
//!   `currently_editing_player: Arc<Mutex<Option<Uuid>>>`（再 1 盒）；
//!   信标 `payment`、唱片机 `record_stack`、讲台 `book`、三叉戟
//!   `item_stack` 均为 `Arc<Mutex<ItemStack>>`（各 1 盒）；
//! - 新态：统一降为裸 `Mutex<T>`（锁语义不变，`ItemStack`/
//!   数组内联进实体本体，引用计数盒整个消失）。
//!
//! 负载：20000 空白牌 + 8000 物品持有者（信标/唱片机/讲台/三叉戟
//! 各 2000）；读电池 20 轮（编辑者/消息内容/物品计数折叠）；
//! 10% 牌单行写。
//!
//! 语义指纹：读写折叠值全等（双臂共用脚本）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use papokin_data::item_stack::ItemStack;

const SIGN_COUNT: usize = 20000;
const HOLDER_COUNT: usize = 8000;
const READ_ROUNDS: usize = 20;

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

fn empty_messages() -> [Box<str>; 4] {
    [Box::from(""), Box::from(""), Box::from(""), Box::from("")]
}

/// 旧态告示牌面：`Arc<Mutex<[Box<str>; 4]>>` 双数组。
struct OldSignText {
    messages: Arc<Mutex<[Box<str>; 4]>>,
    filtered_messages: Arc<Mutex<[Box<str>; 4]>>,
    _glowing: AtomicBool,
    _color: AtomicI8,
}

struct OldSign {
    front: OldSignText,
    back: OldSignText,
    editing: Arc<Mutex<Option<u128>>>,
}

fn old_sign() -> OldSign {
    let side = || OldSignText {
        messages: Arc::new(Mutex::new(empty_messages())),
        filtered_messages: Arc::new(Mutex::new(empty_messages())),
        _glowing: AtomicBool::new(false),
        _color: AtomicI8::new(0),
    };
    OldSign {
        front: side(),
        back: side(),
        editing: Arc::new(Mutex::new(None)),
    }
}

/// 新态告示牌面：裸 `Mutex` 数组。
struct NewSignText {
    messages: Mutex<[Box<str>; 4]>,
    filtered_messages: Mutex<[Box<str>; 4]>,
    _glowing: AtomicBool,
    _color: AtomicI8,
}

struct NewSign {
    front: NewSignText,
    back: NewSignText,
    editing: Mutex<Option<u128>>,
}

fn new_sign() -> NewSign {
    let side = || NewSignText {
        messages: Mutex::new(empty_messages()),
        filtered_messages: Mutex::new(empty_messages()),
        _glowing: AtomicBool::new(false),
        _color: AtomicI8::new(0),
    };
    NewSign {
        front: side(),
        back: side(),
        editing: Mutex::new(None),
    }
}

/// 旧态物品持有者（信标 `payment` / 唱片机 `record_stack` / 讲台
/// `book` / 三叉戟 `item_stack` 的同构形态）。
struct OldHolder {
    stack: Arc<Mutex<ItemStack>>,
}

struct NewHolder {
    stack: Mutex<ItemStack>,
}

const FP_SEED: u64 = 0xcbf2_9ce4_8422_2325;

const fn fp_mix(fp: u64, x: u64) -> u64 {
    (fp ^ x).wrapping_mul(0x1000_0000_01b3)
}

fn fold_str(fp: u64, s: &str) -> u64 {
    s.bytes().fold(fp, |acc, b| fp_mix(acc, u64::from(b)))
}

fn lock_lines(lock: &Mutex<[Box<str>; 4]>) -> std::sync::MutexGuard<'_, [Box<str>; 4]> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// 双臂
// ---------------------------------------------------------------------------

struct ArmStats {
    construct_allocs: usize,
    construct_ms: f64,
    read_ms: f64,
    write_allocs: usize,
    resident: usize,
}

fn old_read_battery(signs: &[OldSign], holders: &[OldHolder], fp: &mut u64) {
    for s in signs {
        *fp = fold_str(*fp, &lock_lines(&s.front.messages)[1]);
        *fp = fold_str(*fp, &lock_lines(&s.back.filtered_messages)[2]);
        let editing_none = s
            .editing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none();
        *fp = fp_mix(*fp, u64::from(u8::from(editing_none)));
    }
    for h in holders {
        let count = h
            .stack
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .item_count;
        *fp = fp_mix(*fp, u64::from(count));
    }
}

fn old_write_phase(signs: &mut [OldSign], fp: &mut u64) {
    for (i, s) in signs.iter_mut().enumerate() {
        if i.is_multiple_of(10) {
            let line: Box<str> = format!("第{i}行").into_boxed_str();
            lock_lines(&s.front.messages)[1] = line.clone();
            lock_lines(&s.front.filtered_messages)[1] = line;
            *fp = fold_str(*fp, &lock_lines(&s.front.messages)[1]);
        }
    }
}

fn run_old_arm() -> (u64, ArmStats) {
    let a0 = allocs();
    let start = Instant::now();
    let mut signs: Vec<OldSign> = (0..SIGN_COUNT).map(|_| old_sign()).collect();
    let holders: Vec<OldHolder> = (0..HOLDER_COUNT)
        .map(|_| OldHolder {
            stack: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
        })
        .collect();
    let construct_allocs = allocs() - a0;
    let construct_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = FP_SEED;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        old_read_battery(&signs, &holders, &mut fp);
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;

    let a1 = allocs();
    old_write_phase(&mut signs, &mut fp);
    let write_allocs = allocs() - a1;
    for s in &signs {
        fp = fold_str(fp, &lock_lines(&s.front.messages)[1]);
    }

    (
        fp,
        ArmStats {
            construct_allocs,
            construct_ms,
            read_ms,
            write_allocs,
            resident: resident(),
        },
    )
}

fn new_read_battery(signs: &[NewSign], holders: &[NewHolder], fp: &mut u64) {
    for s in signs {
        *fp = fold_str(*fp, &lock_lines(&s.front.messages)[1]);
        *fp = fold_str(*fp, &lock_lines(&s.back.filtered_messages)[2]);
        let editing_none = s
            .editing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none();
        *fp = fp_mix(*fp, u64::from(u8::from(editing_none)));
    }
    for h in holders {
        let count = h
            .stack
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .item_count;
        *fp = fp_mix(*fp, u64::from(count));
    }
}

fn new_write_phase(signs: &mut [NewSign], fp: &mut u64) {
    for (i, s) in signs.iter_mut().enumerate() {
        if i.is_multiple_of(10) {
            let line: Box<str> = format!("第{i}行").into_boxed_str();
            lock_lines(&s.front.messages)[1] = line.clone();
            lock_lines(&s.front.filtered_messages)[1] = line;
            *fp = fold_str(*fp, &lock_lines(&s.front.messages)[1]);
        }
    }
}

fn run_new_arm() -> (u64, ArmStats) {
    let a0 = allocs();
    let start = Instant::now();
    let mut signs: Vec<NewSign> = (0..SIGN_COUNT).map(|_| new_sign()).collect();
    let holders: Vec<NewHolder> = (0..HOLDER_COUNT)
        .map(|_| NewHolder {
            stack: Mutex::new(ItemStack::EMPTY.clone()),
        })
        .collect();
    let construct_allocs = allocs() - a0;
    let construct_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = FP_SEED;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        new_read_battery(&signs, &holders, &mut fp);
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;

    let a1 = allocs();
    new_write_phase(&mut signs, &mut fp);
    let write_allocs = allocs() - a1;
    for s in &signs {
        fp = fold_str(fp, &lock_lines(&s.front.messages)[1]);
    }

    (
        fp,
        ArmStats {
            construct_allocs,
            construct_ms,
            read_ms,
            write_allocs,
            resident: resident(),
        },
    )
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn main() {
    println!("轮次 48 基准：方块实体与投射物冗余 Arc 盒去化");
    println!(
        "语料：{SIGN_COUNT} 空白牌 + {HOLDER_COUNT} 物品持有者 · 读 {READ_ROUNDS} 轮 · 10% 牌单行写"
    );
    println!();

    let (old_fp, old) = run_old_arm();
    let (new_fp, new) = run_new_arm();
    let v = evaluate(&old, &new, old_fp, new_fp);
    print_gates(&v, &old, &new, old_fp, new_fp);
    write_report(&v, &old, &new, old_fp, new_fp);
}

struct Verdict {
    semantics_ok: bool,
    resident_ok: bool,
    construct_allocs_ok: bool,
    write_ok: bool,
    read_ok: bool,
    read_ratio: f64,
    all_passed: bool,
}

fn evaluate(old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let resident_ok = new.resident < old.resident;
    let construct_allocs_ok = new.construct_allocs < old.construct_allocs;
    let write_ok = new.write_allocs == old.write_allocs;
    let read_ratio = old.read_ms / new.read_ms;
    let read_ok = read_ratio >= 0.91;
    Verdict {
        semantics_ok,
        resident_ok,
        construct_allocs_ok,
        write_ok,
        read_ok,
        read_ratio,
        all_passed: semantics_ok && resident_ok && construct_allocs_ok && write_ok && read_ok,
    }
}

fn print_gates(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    let total = SIGN_COUNT + HOLDER_COUNT;
    println!("=== 闸门 ===");
    println!(
        "{} 读写电池指纹全等：旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}",
        mark(v.semantics_ok)
    );
    println!(
        "{} 驻留字节下降（主判据）：{} → {} B（每实例均摊 {:.1} → {:.1} B）",
        mark(v.resident_ok),
        old.resident,
        new.resident,
        old.resident as f64 / total as f64,
        new.resident as f64 / total as f64
    );
    println!(
        "{} 构造分配次数下降：{} → {} 次",
        mark(v.construct_allocs_ok),
        old.construct_allocs,
        new.construct_allocs
    );
    println!(
        "{} 写路径不回归（等分配）：{} → {} 次",
        mark(v.write_ok),
        old.write_allocs,
        new.write_allocs
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
        "round": 48,
        "topic": "redundant_arc_unwrap",
        "corpus": {
            "signs": SIGN_COUNT,
            "holders": HOLDER_COUNT,
            "read_rounds": READ_ROUNDS,
            "de_arced_fields": ["sign_messages", "sign_filtered_messages",
                "sign_editing_player", "beacon_payment", "jukebox_record",
                "lectern_book", "trident_item_stack"],
        },
        "gates": [
            {"name": "读写电池指纹全等", "passed": v.semantics_ok,
             "detail": format!("0x{old_fp:016x} vs 0x{new_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", old.resident, new.resident)},
            {"name": "构造分配次数下降", "passed": v.construct_allocs_ok,
             "detail": format!("{} → {}", old.construct_allocs, new.construct_allocs)},
            {"name": "写路径不回归（等分配）", "passed": v.write_ok,
             "detail": format!("{} → {}", old.write_allocs, new.write_allocs)},
            {"name": "读耗时不劣化（≥0.91×）", "passed": v.read_ok,
             "detail": format!("×{:.3}", v.read_ratio)},
        ],
        "old": {
            "resident_bytes": old.resident,
            "construct_allocs": old.construct_allocs,
            "construct_ms": old.construct_ms,
            "read_ms": old.read_ms,
            "write_allocs": old.write_allocs,
            "content_fingerprint": format!("0x{old_fp:016x}"),
        },
        "new": {
            "resident_bytes": new.resident,
            "construct_allocs": new.construct_allocs,
            "construct_ms": new.construct_ms,
            "read_ms": new.read_ms,
            "write_allocs": new.write_allocs,
            "content_fingerprint": format!("0x{new_fp:016x}"),
        },
        "sizes": {
            "arc_mutex_str4_bytes": std::mem::size_of::<Arc<Mutex<[Box<str>; 4]>>>(),
            "mutex_str4_bytes": std::mem::size_of::<Mutex<[Box<str>; 4]>>(),
            "arc_mutex_itemstack_bytes": std::mem::size_of::<Arc<Mutex<ItemStack>>>(),
            "mutex_itemstack_bytes": std::mem::size_of::<Mutex<ItemStack>>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round48-redundant-arc-unwrap.json");
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
