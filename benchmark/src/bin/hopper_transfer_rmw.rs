//! 轮次 38 基准：漏斗传输路径的锁稳定读改写（RMW）。
//!
//! 对照形态（`add_one_item` 的「`get_stack` 克隆 → 改写 →
//! `set_stack` 回写」两段锁读改写，与轮 34 注释的并发窗口同源）：
//! - 旧臂（现状）：每个候选槽先整栈克隆（1 Vec + 每组件一次
//!   dyn 克隆，模型记 1 Vec + 2 String），改写后经 `set_stack`
//!   回写——合并路径付整栈克隆，**满载扫描对每个非空槽白付
//!   克隆后丢弃**，读与写之间还存在并发替换窗口；
//! - 新臂（收编）：`with_stack_mut` 在与 `set_stack` 等同的
//!   写锁内原位改写——合并零克隆、满载扫描零克隆、空槽填入
//!   仅付一次 `item` 克隆（两臂同付，物品必须入槽）。
//!
//! 负载模型（「自动物品传输」混合轮，每轮 1 个漏斗周期）：
//! - 每轮 1 次成功合并（目标槽非空同物未满，+1）；
//! - 每 8 轮一次满载失败扫描（5 槽全非空异物，逐槽克隆后
//!   判不可合并）+ 合成器弹出后 9 槽扣减（3 非空槽克隆后
//!   各 -1，6 空槽零分配）；
//! - 每 16 轮一次空槽填入（整栈移入，两臂各付 1 次 item
//!   克隆）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 两臂语义指纹一致（成功合并/失败扫描/填入/扣减折叠）；
//! - G2 旧臂 ≥ 轮数×3 + (轮数/8)×24 + (轮数/16)×3、
//!   新臂 == 3（仅空槽首填一次 item 克隆）；
//! - G3 新臂耗时 ≤ 旧臂；
//! - G4 旧臂毛字节 > 新臂。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// 分配计数器（次数 + 毛字节 + 释放字节）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREED_BYTES: AtomicU64 = AtomicU64::new(0);

struct Counting;

// SAFETY: 仅转发 System 并计数，不改动布局与返回值语义。
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(
            u64::try_from(layout.size()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(
            u64::try_from(layout.size()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// 阶段读数：分配次数增量 + 毛字节增量。
type Delta = (u64, u64);

fn snapshot() -> (u64, u64) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
    )
}

fn delta_since(start: (u64, u64)) -> Delta {
    let end = snapshot();
    (end.0 - start.0, end.1 - start.1)
}

const fn verdict(pass: bool) -> &'static str {
    if pass { "PASS" } else { "FAIL" }
}

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 混合轮数。
const N_ROUNDS: u64 = 100_000;
/// 满载扫描与合成器扣减周期（漏斗 8 刻冷却）。
const FULL_EVERY: u64 = 8;
/// 空槽填入周期（新目标容器接入节奏）。
const FILL_EVERY: u64 = 16;
/// 每次整栈克隆的分配数（1 Vec + 2 String 组件克隆）。
const ALLOCS_PER_CLONE: u64 = 3;
/// 满载扫描槽位数（漏斗 5 槽）。
const FULL_SLOTS: u64 = 5;
/// 合成器扣减扫描槽位中非空槽数（9 槽 3 非空）。
const CRAFTER_OCCUPIED: u64 = 3;
/// 填入槽索引（槽 5 空备用位的后继）。
const FILL_SLOT: u64 = 6;

/// 模型物品栈（与轮次 37 bin 同形）：克隆面 = 1 Vec + 每组件
/// 一次堆分配。
#[derive(Clone)]
struct ModelStack {
    item_id: u32,
    count: u8,
    #[allow(dead_code)] // 保真复刻：补丁字段随结构驻留但不参与本电池读路径
    patch: Option<Vec<String>>,
}

impl ModelStack {
    const fn is_empty(&self) -> bool {
        self.count == 0
    }

    const fn same_item(&self, other: &Self) -> bool {
        self.item_id == other.item_id
    }
}

/// 模型容器：RwLock<Vec<..>>（镜像漏斗/箱子类 `items` 字段）。
struct ModelInv {
    items: RwLock<Vec<ModelStack>>,
}

impl ModelInv {
    const fn new(slots: Vec<ModelStack>) -> Self {
        Self {
            items: RwLock::new(slots),
        }
    }

    /// 旧臂镜像：`get_stack`——锁内读 + 整栈克隆。
    fn get_stack(&self, slot: usize) -> ModelStack {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot].clone()
    }

    /// 旧臂镜像：`set_stack`——写锁 + 移入。
    fn set_stack(&self, slot: usize, stack: ModelStack) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot] = stack;
    }

    /// 新臂镜像：`with_stack_mut`——写锁内原位改写。
    fn with_stack_mut(&self, slot: usize, f: &mut dyn FnMut(&mut ModelStack) -> bool) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = f(&mut items[slot]);
    }
}

/// 传输用物品（带 2 组件补丁）。
fn transfer_item() -> ModelStack {
    ModelStack {
        item_id: 7,
        count: 1,
        patch: Some(vec!["efficiency=3".to_string(), "unbreaking=2".to_string()]),
    }
}

/// 合并候选槽（非空同物未满）。
fn mergeable_slot() -> ModelStack {
    ModelStack {
        item_id: 7,
        count: 12,
        patch: Some(vec!["efficiency=3".to_string(), "unbreaking=2".to_string()]),
    }
}

/// 满载槽（非空异物）。
fn foreign_slot() -> ModelStack {
    ModelStack {
        item_id: 9,
        count: 64,
        patch: Some(vec!["fortune=2".to_string(), "mending=1".to_string()]),
    }
}

const fn empty_slot() -> ModelStack {
    ModelStack {
        item_id: 0,
        count: 0,
        patch: None,
    }
}

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

/// 旧臂一轮：合并（克隆-改写-回写）+ 周期性满载扫描/扣减/填入。
/// `item` 为预构建传输物品（两臂共享，不进计量差异面）。
fn round_old(inv: &ModelInv, item: &ModelStack, round: u64, fp: u64) -> u64 {
    let mut fp = fp;
    // 每轮 1 次成功合并
    let mut dst = inv.get_stack(0);
    let merged = !dst.is_empty() && dst.same_item(item) && dst.count < 64;
    if merged {
        dst.count += 1;
        inv.set_stack(0, dst);
        fp = fp.wrapping_mul(31).wrapping_add(1);
    }
    black_box(&item);
    if round.is_multiple_of(FULL_EVERY) {
        // 满载失败扫描：5 槽逐槽克隆后判不可合并
        let mut any = false;
        for i in 0..FULL_SLOTS as usize {
            let dst = inv.get_stack(i);
            if dst.is_empty() || dst.same_item(item) {
                any = true;
            }
        }
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(any));
        // 合成器扣减：3 个非空槽各克隆后 -1（槽 0 每轮净增，
        // 永不枯竭，忠实于「合成产出-弹出消耗」稳态）
        for _ in 0..CRAFTER_OCCUPIED {
            let mut stack = inv.get_stack(0);
            if !stack.is_empty() {
                stack.count -= 1;
                inv.set_stack(0, stack);
                fp = fp.wrapping_mul(31).wrapping_add(2);
            }
        }
    }
    if round.is_multiple_of(FILL_EVERY) {
        // 填入槽单槽传输（add_one_item 语义）：空则整栈移入
        // （两臂同付 item 克隆），非空同物未满则 +1
        let dst = inv.get_stack(FILL_SLOT as usize);
        if dst.is_empty() {
            inv.set_stack(FILL_SLOT as usize, item.clone());
            fp = fp.wrapping_mul(31).wrapping_add(3);
        } else if dst.same_item(item) && dst.count < 64 {
            let mut dst = dst;
            dst.count += 1;
            inv.set_stack(FILL_SLOT as usize, dst);
            fp = fp.wrapping_mul(31).wrapping_add(3);
        }
    }
    fp
}

/// 新臂一轮：`with_stack_mut` 原位读改写。
fn round_new(inv: &ModelInv, item: &ModelStack, round: u64, fp: u64) -> u64 {
    let mut fp = fp;
    // 每轮 1 次成功合并（原位 +1，零克隆）
    let mut merged = false;
    inv.with_stack_mut(0, &mut |dst| {
        let ok = !dst.is_empty() && dst.same_item(item) && dst.count < 64;
        if ok {
            dst.count += 1;
            merged = true;
        }
        ok
    });
    if merged {
        fp = fp.wrapping_mul(31).wrapping_add(1);
    }
    black_box(&item);
    if round.is_multiple_of(FULL_EVERY) {
        // 满载失败扫描：借用判读，零克隆
        let mut any = false;
        for i in 0..FULL_SLOTS as usize {
            inv.with_stack_mut(i, &mut |dst| {
                if dst.is_empty() || dst.same_item(item) {
                    any = true;
                }
                false
            });
        }
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(any));
        // 合成器扣减：原位 -1，零克隆（与旧臂同槽稳态）
        for _ in 0..CRAFTER_OCCUPIED {
            let mut decremented = false;
            inv.with_stack_mut(0, &mut |stack| {
                if stack.is_empty() {
                    false
                } else {
                    stack.count -= 1;
                    decremented = true;
                    true
                }
            });
            if decremented {
                fp = fp.wrapping_mul(31).wrapping_add(2);
            }
        }
    }
    if round.is_multiple_of(FILL_EVERY) {
        // 填入槽单槽传输：空槽首填付一次 item 克隆（与旧臂同
        // 付），此后合并在位 +1 零克隆
        let mut transferred = false;
        inv.with_stack_mut(FILL_SLOT as usize, &mut |dst| {
            if dst.is_empty() {
                *dst = item.clone();
                transferred = true;
                true
            } else if dst.same_item(item) && dst.count < 64 {
                dst.count += 1;
                transferred = true;
                true
            } else {
                false
            }
        });
        if transferred {
            fp = fp.wrapping_mul(31).wrapping_add(3);
        }
    }
    fp
}

/// 起始槽位布局：槽 0 同物未满、槽 1-4 满载异物、槽 5 空、
/// 槽 6+ 空备用。
fn base_slots() -> Vec<ModelStack> {
    let mut slots = vec![empty_slot(); 8];
    slots[0] = mergeable_slot();
    for slot in slots.iter_mut().take(FULL_SLOTS as usize).skip(1) {
        *slot = foreign_slot();
    }
    slots
}

fn arm(round: fn(&ModelInv, &ModelStack, u64, u64) -> u64) -> Report {
    let inv = ModelInv::new(base_slots());
    // 预构建传输物品：两臂共享同一引用，构造分配不进差异面
    let item = transfer_item();
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for i in 0..N_ROUNDS {
        fp = round(&inv, &item, i, fp);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn main() {
    let mut gates_pass = true;

    println!("== 轮次 38 双臂基准：漏斗传输路径锁稳定读改写 ==");
    println!(
        "负载：{N_ROUNDS} 混合轮（每轮合并；每 {FULL_EVERY} 轮满载扫描 5 槽+合成器扣减 3 槽；每 {FILL_EVERY} 轮空槽填入），模型容器 8 槽\n"
    );

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    let old = arm(round_old);
    let new = arm(round_new);
    println!(
        "旧臂 {:>8} allocs / {:>9} B / {:>10} ns",
        old.delta.0, old.delta.1, old.ns
    );
    println!(
        "新臂 {:>8} allocs / {:>9} B / {:>10} ns",
        new.delta.0, new.delta.1, new.ns
    );

    let g1 = old.fp == new.fp;
    println!(
        "G1 两臂语义指纹一致（{:#x} vs {:#x}）→ {}",
        old.fp,
        new.fp,
        verdict(g1)
    );
    gates_pass &= g1;
    let old_floor = N_ROUNDS * ALLOCS_PER_CLONE
        + (N_ROUNDS / FULL_EVERY) * (FULL_SLOTS + CRAFTER_OCCUPIED) * ALLOCS_PER_CLONE
        + (N_ROUNDS / FILL_EVERY) * ALLOCS_PER_CLONE;
    let new_expect = ALLOCS_PER_CLONE;
    let g2 = old.delta.0 >= old_floor && new.delta.0 == new_expect;
    println!(
        "G2 旧臂 ≥ {}（实际 {}）、新臂 == {}（实际 {}）→ {}",
        old_floor,
        old.delta.0,
        new_expect,
        new.delta.0,
        verdict(g2)
    );
    gates_pass &= g2;
    let g3 = new.ns <= old.ns;
    println!(
        "G3 新臂耗时 ≤ 旧臂（{} ns ≤ {} ns）→ {}",
        new.ns,
        old.ns,
        verdict(g3)
    );
    gates_pass &= g3;
    let g4 = old.delta.1 > new.delta.1;
    println!(
        "G4 旧臂毛字节 > 新臂（{} B > {} B）→ {}",
        old.delta.1,
        new.delta.1,
        verdict(g4)
    );
    gates_pass &= g4;

    println!(
        "\n门禁汇总：{}",
        if gates_pass {
            "全部 PASS"
        } else {
            "存在 FAIL（见上）"
        }
    );
    std::process::exit(i32::from(!gates_pass));
}
