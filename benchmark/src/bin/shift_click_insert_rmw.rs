//! 轮次 39 基准：屏幕处理器插入引擎的锁稳定读改写。
//!
//! 对照形态（`insert_item` 两段锁「克隆-改写-回写」，18 个屏幕
//! 处理器共用的 shift-click / 结果取出插入核心）：
//! - 旧臂（现状）：合并趟对每个占用槽 `get_stack` 整栈克隆（1
//!   Vec + 每组件一次 dyn 克隆，模型记 1 Vec + 2 String）再判
//!   可否合并——**同物未满才真正改写，异物/已满槽的克隆全部
//!   白付**；`can_fully_insert` 预检逐槽克隆只为算容量；
//!   `QuickMove` 重试环整栈克隆只为与源槽等值比较；
//! - 新臂（收编）：`Slot::with_stack_mut` 透传物品栏写锁内
//!   原位合并（零克隆），空槽趟借用探测 + 回调内复查原位置入
//!   （split 成本两臂同付），预检与重试环借用判读零克隆。
//!
//! 负载模型（「shift-click 整理」混合轮，36 槽区域）：
//! - 每轮 1 次 shift-click：合并趟扫 10 占用槽（6 异物 + 4
//!   同物未满，旧臂 10 克隆）→ 空槽趟首空位置入（split 两臂
//!   同付 3 分配）→ 重试环 2 次源槽探测（旧臂 2 克隆）；
//! - 每 8 轮一次结果取出：`can_fully_insert` 预检 10 占用槽
//!   （旧臂 10 克隆）+ 结果堆合并插入 4 同物槽（旧臂 4 克隆）；
//! - 每轮末消费排空回基线（直写脚手架，两臂同构零分配）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 两臂语义指纹一致（合并量/置入量/重试探测/结果取出折叠）；
//! - G2 旧臂 ≥ 轮数×39 + (轮数/8)×66、新臂 == 轮数×3 +
//!   (轮数/8)×6（空槽 split 与结果事件构造两臂同付）；
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
/// 结果取出周期（合成节奏）。
const RESULT_EVERY: u64 = 8;
/// 合并趟扫描的占用槽数（6 异物 + 4 同物）。
const OCCUPIED_SLOTS: usize = 10;
/// 同物槽起始索引。
const MATCH_START: usize = 6;
/// 区域总槽数。
const TOTAL_SLOTS: usize = 36;
/// 每次整栈克隆的分配数（1 Vec + 2 String 组件克隆）。
const ALLOCS_PER_CLONE: u64 = 3;
/// 每轮旧臂保底：合并趟 10 + split 1 + 重试探测 2。
const OLD_PER_ROUND: u64 = (OCCUPIED_SLOTS as u64 + 1 + 2) * ALLOCS_PER_CLONE;
/// 结果事件旧臂保底：预检 10 + 结果合并 10 + needle/结果堆构造 2
/// （主合并趟后同物槽已满，预检与结果合并不早停）。
const OLD_PER_RESULT: u64 = (OCCUPIED_SLOTS as u64 * 2 + 2) * ALLOCS_PER_CLONE;
/// 新臂结果事件保底：needle + 结果堆构造（split 同付）。
const NEW_PER_RESULT: u64 = 2 * ALLOCS_PER_CLONE;

/// 模型物品栈：克隆面 = 1 Vec + 每组件一次堆分配。
#[derive(Clone)]
struct ModelStack {
    item_id: u32,
    count: u8,
    /// 分配面模型：仅经克隆路径保活，不直接读取
    #[allow(dead_code)]
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

/// 构造带 2 组件的新栈（split 的分配形态，两臂同付）。
fn make_stack(item_id: u32, count: u8) -> ModelStack {
    ModelStack {
        item_id,
        count,
        patch: Some(vec!["efficiency=3".to_string(), "unbreaking=2".to_string()]),
    }
}

const fn empty_slot() -> ModelStack {
    ModelStack {
        item_id: 0,
        count: 0,
        patch: None,
    }
}

/// 模型容器：RwLock<Vec<..>>（镜像箱子类物品栏）。
struct ModelInv {
    items: RwLock<Vec<ModelStack>>,
}

impl ModelInv {
    fn new() -> Self {
        let mut slots = vec![empty_slot(); TOTAL_SLOTS];
        for (i, slot) in slots.iter_mut().enumerate() {
            if i < MATCH_START {
                *slot = make_stack(9, 64); // 异物满载
            } else if i < OCCUPIED_SLOTS {
                *slot = make_stack(7, 50); // 同物未满（容量 14）
            }
        }
        Self {
            items: RwLock::new(slots),
        }
    }

    fn get_stack(&self, slot: usize) -> ModelStack {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot].clone()
    }

    fn set_stack(&self, slot: usize, stack: ModelStack) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot] = stack;
    }

    fn with_stack(&self, slot: usize, f: &mut dyn FnMut(&ModelStack)) {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&items[slot]);
    }

    fn with_stack_mut(&self, slot: usize, f: &mut dyn FnMut(&mut ModelStack) -> bool) -> bool {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut items[slot])
    }

    /// 消费排空回基线（共享脚手架：直写零分配，两臂同构）。
    fn drain_to_baseline(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (i, slot) in items.iter_mut().enumerate() {
            if (MATCH_START..OCCUPIED_SLOTS).contains(&i) {
                slot.count = 50;
            } else if i >= OCCUPIED_SLOTS {
                *slot = empty_slot();
            }
        }
    }
}

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

/// 旧臂合并趟：逐占用槽克隆后判定，命中才改写。
fn merge_pass_old(inv: &ModelInv, src: &mut ModelStack, fp: u64) -> u64 {
    let mut fp = fp;
    for i in 0..OCCUPIED_SLOTS {
        let mut dst = inv.get_stack(i);
        let merged = i >= MATCH_START && dst.same_item(src) && dst.count < 64;
        if merged {
            let take = src.count.min(64 - dst.count);
            dst.count += take;
            src.count -= take;
            inv.set_stack(i, dst);
            fp = fp.wrapping_mul(31).wrapping_add(u64::from(take));
        }
        if src.count == 0 {
            break;
        }
    }
    fp
}

/// 新臂合并趟：原位合并零克隆。
fn merge_pass_new(inv: &ModelInv, src: &mut ModelStack, fp: u64) -> u64 {
    let mut fp = fp;
    for i in 0..OCCUPIED_SLOTS {
        let mut taken = 0u8;
        inv.with_stack_mut(i, &mut |dst| {
            let ok = i >= MATCH_START && dst.same_item(src) && dst.count < 64;
            if ok {
                taken = src.count.min(64 - dst.count);
                dst.count += taken;
                src.count -= taken;
                true
            } else {
                false
            }
        });
        if taken > 0 {
            fp = fp.wrapping_mul(31).wrapping_add(u64::from(taken));
        }
        if src.count == 0 {
            break;
        }
    }
    fp
}

/// 旧臂空槽趟：克隆探测（空槽零分配）+ split 置入。
fn empty_pass_old(inv: &ModelInv, src: &mut ModelStack, fp: u64) -> u64 {
    let mut fp = fp;
    if src.count > 0 {
        for i in OCCUPIED_SLOTS..TOTAL_SLOTS {
            let dst = inv.get_stack(i);
            if dst.is_empty() {
                let n = src.count;
                src.count = 0;
                inv.set_stack(i, make_stack(7, n));
                fp = fp.wrapping_mul(31).wrapping_add(u64::from(n) + 1);
                break;
            }
        }
    }
    fp
}

/// 新臂空槽趟：借用探测 + 回调内复查原位置入（split 同付）。
fn empty_pass_new(inv: &ModelInv, src: &mut ModelStack, fp: u64) -> u64 {
    let mut fp = fp;
    if src.count > 0 {
        'slots: for i in OCCUPIED_SLOTS..TOTAL_SLOTS {
            let mut empty = false;
            inv.with_stack(i, &mut |dst| empty = dst.is_empty());
            if empty {
                let mut placed_n = 0u8;
                inv.with_stack_mut(i, &mut |dst| {
                    if dst.is_empty() {
                        placed_n = src.count;
                        src.count = 0;
                        *dst = make_stack(7, placed_n);
                        true
                    } else {
                        false
                    }
                });
                if placed_n > 0 {
                    fp = fp.wrapping_mul(31).wrapping_add(u64::from(placed_n) + 1);
                    break 'slots;
                }
            }
        }
    }
    fp
}

/// 旧臂一轮 shift-click。
fn round_old(inv: &ModelInv, src: &mut ModelStack, round: u64, fp: u64) -> u64 {
    let mut fp = fp;
    fp = merge_pass_old(inv, src, fp);
    fp = empty_pass_old(inv, src, fp);
    // 重试环探测 ×2（整栈克隆只为等值判读）
    let probe_a = inv.get_stack(MATCH_START);
    let probe_b = inv.get_stack(MATCH_START);
    fp = fp
        .wrapping_mul(31)
        .wrapping_add(u64::from(probe_a.item_id & 1));
    fp = fp
        .wrapping_mul(31)
        .wrapping_add(u64::from(probe_b.count & 1));
    if round.is_multiple_of(RESULT_EVERY) {
        // 结果取出：预检 10 槽逐克隆 + 结果堆合并（needle 与结果
        // 堆构造两臂同付）
        let needle = make_stack(7, 1);
        let mut remaining = 8u8;
        for i in 0..OCCUPIED_SLOTS {
            let existing = inv.get_stack(i);
            let capacity = if existing.is_empty() {
                64
            } else if existing.same_item(&needle) {
                64u8.saturating_sub(existing.count)
            } else {
                0
            };
            remaining = remaining.saturating_sub(capacity);
            if remaining == 0 {
                break;
            }
        }
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(remaining));
        let mut result = make_stack(7, 8);
        fp = merge_pass_old(inv, &mut result, fp);
    }
    inv.drain_to_baseline();
    fp
}

/// 新臂一轮 shift-click。
fn round_new(inv: &ModelInv, src: &mut ModelStack, round: u64, fp: u64) -> u64 {
    let mut fp = fp;
    fp = merge_pass_new(inv, src, fp);
    fp = empty_pass_new(inv, src, fp);
    // 重试环探测 ×2（借用判读零克隆；折叠序与旧臂一致）
    let mut probe = 0u32;
    inv.with_stack(MATCH_START, &mut |source| probe = source.item_id & 1);
    fp = fp.wrapping_mul(31).wrapping_add(u64::from(probe));
    inv.with_stack(MATCH_START, &mut |source| {
        probe = u32::from(source.count & 1);
    });
    fp = fp.wrapping_mul(31).wrapping_add(u64::from(probe));
    if round.is_multiple_of(RESULT_EVERY) {
        // 结果取出：借用预检 + 原位合并（needle 与结果堆构造
        // 两臂同付）
        let needle = make_stack(7, 1);
        let mut remaining = 8u8;
        for i in 0..OCCUPIED_SLOTS {
            inv.with_stack(i, &mut |existing| {
                let capacity = if existing.is_empty() {
                    64
                } else if existing.same_item(&needle) {
                    64u8.saturating_sub(existing.count)
                } else {
                    0
                };
                remaining = remaining.saturating_sub(capacity);
            });
            if remaining == 0 {
                break;
            }
        }
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(remaining));
        let mut result = make_stack(7, 8);
        fp = merge_pass_new(inv, &mut result, fp);
    }
    inv.drain_to_baseline();
    fp
}

fn arm(round: fn(&ModelInv, &mut ModelStack, u64, u64) -> u64) -> Report {
    let inv = ModelInv::new();
    let mut src = make_stack(7, 64);
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for i in 0..N_ROUNDS {
        src.count = 64; // 源堆复位（直写脚手架，两臂同构）
        fp = round(&inv, &mut src, i, fp);
    }
    let delta = delta_since(start);
    black_box(&src);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn main() {
    let mut gates_pass = true;

    println!("== 轮次 39 双臂基准：屏幕处理器插入引擎锁稳定读改写 ==");
    println!(
        "负载：{N_ROUNDS} 混合轮（每轮 shift-click 合并趟 10 占用槽+空槽置入+重试探测 2；每 {RESULT_EVERY} 轮结果取出预检+合并），36 槽区域\n"
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
    let old_floor = N_ROUNDS * OLD_PER_ROUND + (N_ROUNDS / RESULT_EVERY) * OLD_PER_RESULT;
    let new_expect = N_ROUNDS * ALLOCS_PER_CLONE + (N_ROUNDS / RESULT_EVERY) * NEW_PER_RESULT;
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
