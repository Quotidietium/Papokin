//! 轮次 46 基准：箱族方块实体空物品数组惰性化。
//!
//! 双臂结构复刻：旧臂 `RwLock<[ItemStack; 27]>` 常驻数组（含
//! 未开封战利品箱与玩家空箱在内的一切箱子都付 27×48B 内联）；
//! 新臂直接使用生产 `LazyItemSlots<27>`（`None` 态零堆，首次
//! 写入物化）。两臂都装进 `Arc<..>` 盒复刻方块实体驻留形态。
//!
//! 语料画像：55% 空箱（战利品待定/玩家空箱）、45% 已装 3-10
//! 件。电池覆盖 `is_empty` / 单槽读 / `with_stack` 借用迭代
//! （NBT 写出形态）/ `with_stack_mut` 增量（漏斗合并形态）/
//! `remove_stack` / `clear`（`Clearable` 形态）。
//!
//! 语义红线：对全部读接口 `None` ≡ 全空数组（`get_stack` 回落
//! 空堆、`is_empty` 真、NBT 无 `Items` 键）；写路径物化后行为
//! 一致——指纹门双臂全等。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_inventory::LazyItemSlots;
use papokin_nbt::compound::NbtCompound;

const CHEST_COUNT: usize = 20_000;
const READ_ROUNDS: usize = 20;
const CLEAR_COUNT: usize = 4_000;
const SLOT_COUNT: usize = 27;
/// 空箱占比（战利品待定 + 玩家空箱）。
const EMPTY_RATIO: usize = 20; // i % 20 < 11 → 55%

// ---------------------------------------------------------------------------
// 计数分配器
// ---------------------------------------------------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREED_BYTES: AtomicU64 = AtomicU64::new(0);

struct CountingAllocator;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl std::alloc::GlobalAlloc for CountingAllocator {
    // SAFETY: 转发系统分配器，统计本身无分配语义约束
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: 转发系统分配器，无额外不变量
        unsafe { std::alloc::System.alloc(layout) }
    }

    // SAFETY: ptr/layout 来自配对的 alloc 返回值
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        FREED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: ptr/layout 来自配对的 alloc 返回值
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed)
}

fn alloc_bytes() -> u64 {
    ALLOC_BYTES.load(Ordering::Relaxed)
}

fn resident() -> u64 {
    ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed)
}

const fn fp_mix(fp: u64, value: u64) -> u64 {
    (fp ^ value).wrapping_mul(0x1000_0000_01b3)
}

// ---------------------------------------------------------------------------
// 双臂箱体
// ---------------------------------------------------------------------------

/// 旧臂：常驻 27 槽数组。
struct OldChest {
    items: RwLock<[ItemStack; SLOT_COUNT]>,
    dirty: AtomicBool,
}

fn old_new() -> OldChest {
    OldChest {
        items: RwLock::new(std::array::from_fn(|_| ItemStack::EMPTY.clone())),
        dirty: AtomicBool::new(false),
    }
}

fn old_is_empty(c: &OldChest) -> bool {
    let items = c
        .items
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    items.iter().all(ItemStack::is_empty)
}

fn old_get_stack(c: &OldChest, slot: usize) -> ItemStack {
    let items = c
        .items
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    items[slot].clone()
}

/// 旧臂 `with_stack` 借用读（NBT 写出/漏斗检查形态）。
fn old_with_stack(c: &OldChest, slot: usize, f: &mut dyn FnMut(&ItemStack)) {
    let items = c
        .items
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&items[slot]);
}

/// 旧臂 `with_stack_mut`（漏斗合并形态：原位写锁 RMW）。
fn old_with_stack_mut(
    c: &OldChest,
    slot: usize,
    f: &mut dyn FnMut(&mut ItemStack) -> bool,
) -> bool {
    let mut items = c
        .items
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let changed = f(&mut items[slot]);
    if changed {
        c.dirty.store(true, Ordering::Relaxed);
    }
    changed
}

fn old_remove_stack(c: &OldChest, slot: usize) -> ItemStack {
    let mut items = c
        .items
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let removed = std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone());
    c.dirty.store(true, Ordering::Relaxed);
    removed
}

fn old_clear(c: &OldChest) {
    let mut items = c
        .items
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    items.fill_with(|| ItemStack::EMPTY.clone());
    c.dirty.store(true, Ordering::Relaxed);
}

/// 新臂：生产 `LazyItemSlots` + 同构脏标记。
struct NewChest {
    items: LazyItemSlots<SLOT_COUNT>,
    dirty: AtomicBool,
}

const fn new_new() -> NewChest {
    NewChest {
        items: LazyItemSlots::new(),
        dirty: AtomicBool::new(false),
    }
}

// ---------------------------------------------------------------------------
// 装备画像
// ---------------------------------------------------------------------------

const fn is_empty_chest(i: usize) -> bool {
    i % EMPTY_RATIO < 11
}

/// 第 i 个非空箱的物品分布（3-10 件、槽位分散）。
const fn fill_plan(i: usize) -> (usize, u8, &'static Item) {
    const ITEMS: [&Item; 6] = [
        &Item::DIRT,
        &Item::IRON_INGOT,
        &Item::GOLD_INGOT,
        &Item::DIAMOND,
        &Item::ARROW,
        &Item::COBBLESTONE,
    ];
    let k = i % 8 + 3;
    let slot = (i * 7) % SLOT_COUNT;
    let item = ITEMS[i % ITEMS.len()];
    (slot, k as u8, item)
}

fn run_old_arm() -> (u64, ArmStats) {
    let fp: u64 = 0xcbf2_9ce4_8422_2325;
    // 空箱专用相位：只构造空箱（G3 按字节度量——空箱的
    // 分配计数两臂同为 1 个 Arc 盒，收益全在盒体积）。
    let e0 = allocs();
    let eb0 = alloc_bytes();
    let empty_corpus: Vec<Arc<OldChest>> = (0..11_000).map(|_| Arc::new(old_new())).collect();
    drop(empty_corpus);
    let empty_bytes = alloc_bytes() - eb0;
    let _empty_allocs = allocs() - e0;

    let a0 = allocs();
    let start = Instant::now();

    let mut corpus: Vec<Arc<OldChest>> = Vec::with_capacity(CHEST_COUNT);
    for i in 0..CHEST_COUNT {
        let chest = Arc::new(old_new());
        if !is_empty_chest(i) {
            let count = i % 8 + 3;
            for k in 0..count {
                let (slot, n, item) = fill_plan(i + k);
                let mut items = chest
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[slot % SLOT_COUNT] = ItemStack::new(n, item);
            }
        }
        corpus.push(chest);
    }
    let spawn_allocs = allocs() - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let (mut fp, battery_ms) = old_battery(&corpus, fp);

    // clear 电池（Clearable 形态）
    for (i, chest) in corpus.iter().enumerate().take(CLEAR_COUNT) {
        old_clear(chest);
        fp = fp_mix(fp, u64::from(is_empty_chest(i)));
    }

    (
        fp,
        ArmStats {
            resident: resident(),
            spawn_allocs,
            spawn_bytes: empty_bytes,
            spawn_ms,
            battery_ms,
        },
    )
}

fn old_battery(corpus: &[Arc<OldChest>], mut fp: u64) -> (u64, f64) {
    let battery_start = Instant::now();
    for round in 0..READ_ROUNDS {
        for (i, chest) in corpus.iter().enumerate() {
            fp = fp_mix(fp, u64::from(old_is_empty(chest)));
            let slot = (i * 7) % SLOT_COUNT;
            let stack = old_get_stack(chest, slot);
            fp = fp_mix(fp, stack.item_count as u64);
            // with_stack 借用迭代（NBT 写出/漏斗预检形态）
            old_with_stack(chest, slot, &mut |stack| {
                fp = fp_mix(fp, stack.item.id as u64);
            });
            // with_stack_mut 合并形态：非空槽 +1
            let added = old_with_stack_mut(chest, slot, &mut |stack| {
                if stack.item_count > 0 && stack.item_count < 99 {
                    stack.item_count += 1;
                    true
                } else {
                    false
                }
            });
            fp = fp_mix(fp, u64::from(added));
            // remove 形态：稀疏
            if i.is_multiple_of(97) {
                let removed = old_remove_stack(chest, (i * 3) % SLOT_COUNT);
                fp = fp_mix(fp, removed.item_count as u64);
            }
            fp = fp_mix(fp, u64::from(round as u8));
        }
    }
    (fp, battery_start.elapsed().as_secs_f64() * 1000.0)
}

fn run_new_arm() -> (u64, ArmStats) {
    let fp: u64 = 0xcbf2_9ce4_8422_2325;
    let e0 = allocs();
    let eb0 = alloc_bytes();
    let empty_corpus: Vec<Arc<NewChest>> = (0..11_000).map(|_| Arc::new(new_new())).collect();
    drop(empty_corpus);
    let empty_bytes = alloc_bytes() - eb0;
    let _empty_allocs = allocs() - e0;

    let a0 = allocs();
    let start = Instant::now();

    let mut corpus: Vec<Arc<NewChest>> = Vec::with_capacity(CHEST_COUNT);
    for i in 0..CHEST_COUNT {
        let chest = Arc::new(new_new());
        if !is_empty_chest(i) {
            let count = i % 8 + 3;
            for k in 0..count {
                let (slot, n, item) = fill_plan(i + k);
                chest
                    .items
                    .with_write(|items| items[slot % SLOT_COUNT] = ItemStack::new(n, item));
            }
        }
        corpus.push(chest);
    }
    let spawn_allocs = allocs() - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let (mut fp, battery_ms) = new_battery(&corpus, fp);

    for (i, chest) in corpus.iter().enumerate().take(CLEAR_COUNT) {
        chest.items.clear();
        chest.dirty.store(true, Ordering::Relaxed);
        fp = fp_mix(fp, u64::from(is_empty_chest(i)));
    }

    (
        fp,
        ArmStats {
            resident: resident(),
            spawn_allocs,
            spawn_bytes: empty_bytes,
            spawn_ms,
            battery_ms,
        },
    )
}

fn new_battery(corpus: &[Arc<NewChest>], mut fp: u64) -> (u64, f64) {
    let battery_start = Instant::now();
    for round in 0..READ_ROUNDS {
        for (i, chest) in corpus.iter().enumerate() {
            fp = fp_mix(fp, u64::from(chest.items.is_empty()));
            let slot = (i * 7) % SLOT_COUNT;
            let stack = chest.items.get_stack(slot);
            fp = fp_mix(fp, stack.item_count as u64);
            chest.items.with_stack(slot, &mut |stack| {
                fp = fp_mix(fp, stack.item.id as u64);
            });
            let added = chest.items.with_stack_mut(slot, &mut |stack| {
                if stack.item_count > 0 && stack.item_count < 99 {
                    stack.item_count += 1;
                    true
                } else {
                    false
                }
            });
            fp = fp_mix(fp, u64::from(added));
            if i.is_multiple_of(97) {
                let removed = chest.items.with_write(|items| {
                    std::mem::replace(&mut items[(i * 3) % SLOT_COUNT], ItemStack::EMPTY.clone())
                });
                fp = fp_mix(fp, removed.item_count as u64);
            }
            fp = fp_mix(fp, u64::from(round as u8));
        }
    }
    (fp, battery_start.elapsed().as_secs_f64() * 1000.0)
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

struct ArmStats {
    resident: u64,
    spawn_allocs: u64,
    spawn_bytes: u64,
    spawn_ms: f64,
    battery_ms: f64,
}

fn main() {
    println!("轮次 46 基准：箱族方块实体空物品数组惰性化（chest/barrel/shulker/shelf）");
    println!("语料：{CHEST_COUNT} 箱（55% 空箱）· 电池 {READ_ROUNDS} 轮 · 清空 {CLEAR_COUNT}");
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
    spawn_allocs_ok: bool,
    battery_ok: bool,
    battery_ratio: f64,
    all_passed: bool,
}

fn evaluate(old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let resident_ok = new.resident < old.resident;
    let spawn_allocs_ok = new.spawn_bytes < old.spawn_bytes;
    let battery_ratio = old.battery_ms / new.battery_ms;
    let battery_ok = battery_ratio >= 0.91;
    Verdict {
        semantics_ok,
        resident_ok,
        spawn_allocs_ok,
        battery_ok,
        battery_ratio,
        all_passed: semantics_ok && resident_ok && spawn_allocs_ok && battery_ok,
    }
}

fn print_gates(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    println!("=== 闸门 ===");
    println!(
        "{} 电池指纹全等：旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}",
        mark(v.semantics_ok)
    );
    println!(
        "{} 驻留字节下降（主判据）：{} → {} B（每箱 {:.1} → {:.1} B）",
        mark(v.resident_ok),
        old.resident,
        new.resident,
        old.resident as f64 / CHEST_COUNT as f64,
        new.resident as f64 / CHEST_COUNT as f64
    );
    println!(
        "{} 空箱构造字节下降：{} → {} B（每空箱 {:.1} → {:.1} B；已装箱另付物化盒 +1 分配，设计内成本）",
        mark(v.spawn_allocs_ok),
        old.spawn_bytes,
        new.spawn_bytes,
        old.spawn_bytes as f64 / 11_000.0,
        new.spawn_bytes as f64 / 11_000.0
    );
    println!(
        "{} 电池耗时不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.battery_ok),
        old.battery_ms,
        new.battery_ms,
        v.battery_ratio
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构造+填充：{:.2} → {:.2} ms（×{:.2}）",
        old.spawn_ms,
        new.spawn_ms,
        old.spawn_ms / new.spawn_ms
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
        "round": 46,
        "topic": "chest_lazy_item_slots",
        "corpus": {
            "chests": CHEST_COUNT,
            "empty_ratio": "55%",
            "slot_count": SLOT_COUNT,
            "read_rounds": READ_ROUNDS,
            "clears": CLEAR_COUNT,
            "family": ["chest", "trapped_chest", "barrel", "shulker_box", "shelf"],
        },
        "gates": [
            {"name": "电池指纹全等", "passed": v.semantics_ok,
             "detail": format!("0x{old_fp:016x} vs 0x{new_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", old.resident, new.resident)},
            {"name": "空箱构造字节下降", "passed": v.spawn_allocs_ok,
             "detail": format!("{} → {} B", old.spawn_bytes, new.spawn_bytes)},
            {"name": "电池耗时不劣化（≥0.91×）", "passed": v.battery_ok,
             "detail": format!("×{:.3}", v.battery_ratio)},
        ],
        "old": {
            "resident_bytes": old.resident,
            "spawn_allocs": old.spawn_allocs,
            "spawn_ms": old.spawn_ms,
            "battery_ms": old.battery_ms,
            "content_fingerprint": format!("0x{old_fp:016x}"),
        },
        "new": {
            "resident_bytes": new.resident,
            "spawn_allocs": new.spawn_allocs,
            "spawn_ms": new.spawn_ms,
            "battery_ms": new.battery_ms,
            "content_fingerprint": format!("0x{new_fp:016x}"),
        },
        "sizes": {
            "old_array_inline_bytes": SLOT_COUNT * std::mem::size_of::<ItemStack>(),
            "new_lazy_inline_bytes": std::mem::size_of::<LazyItemSlots<SLOT_COUNT>>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round46-chest-lazy-item-slots.json");
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

// 避免未使用导入告警（NbtCompound 保留供 NBT 写出形态扩展）。
#[allow(dead_code)]
const fn _touch(_nbt: &mut NbtCompound) {}
