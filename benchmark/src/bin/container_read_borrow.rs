//! 轮次 36 基准：容器读取的借用化（`get_stack` 深克隆 → `with_stack` 锁内回调）。
//!
//! 对照形态（轮次 36 收编面：`Inventory` trait 默认方法 3 处 +
//! 漏斗/矿车漏斗 4 处 + 比较器/战利品/书架/合成器/配方放置 6 处，
//! 合计 13 个生产读取点）：
//! - 旧臂（现状）：`get_stack(slot)` 按值返回——`ItemStack` 的
//!   `patch: Vec<(DataComponent, Option<Box<dyn ...>>)>` 逐槽深克隆
//!   （1 次 Vec 分配 + 每组件一次 dyn 克隆分配），纯读用途
//!   （比较器输出/满载判定/配方匹配/计数/空槽探测）全部白付；
//! - 新臂（收编）：`with_stack(slot, f)` 借用读取（20 个容器实现
//!   已覆写为锁内回调），纯读扫描零克隆；空 Vec 的克隆本就零
//!   分配，收益集中在**非空带组件槽**（1 Vec + N 组件/槽）。
//!
//! 负载模型（「容器运转」混合轮；槽位内容跨轮不变，复刻稳态）：
//! - 比较器输出计算：9 槽（4 非空各带 2 组件 + 5 空）——
//!   旧臂 4 × 3 = 12 分配；
//! - 漏斗吐出扫描：5 槽（2 非空带 2 组件 + 3 空，事件无监听）；
//! - 漏斗吸入扫描：上方 27 槽箱子（3 非空 + 24 空）——
//!   两段合计旧臂 5 × 3 = 15 分配；
//! - `count()` 同物品计数：36 槽背包（8 非空）——旧臂 8 × 3 = 24 分配。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 两臂语义指纹一致（输出值/传输判定/计数逐位同构）；
//! - G2 旧臂分配 ≥ 轮数 × 51（每轮 51 个白付分配）、新臂 == 0；
//! - G3 新臂耗时 ≤ 旧臂；
//! - G4 旧臂毛字节 > 新臂（>0 vs 0）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
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
/// 每轮白付分配数（比较器 12 + 漏斗两段 15 + count 24）。
const ALLOCS_PER_ROUND: u64 = 51;

/// 物品堆模型：`patch` 非空即带组件（克隆 = 1 Vec + 每组件 1 分配）。
#[derive(Clone)]
struct ModelStack {
    item_id: u16,
    item_count: u8,
    /// None = 空槽（克隆零分配）；Some(v) 非空，v 逐元素克隆分配。
    patch: Option<Vec<String>>,
}

impl ModelStack {
    const fn empty() -> Self {
        Self {
            item_id: 0,
            item_count: 0,
            patch: None,
        }
    }

    fn filled(item_id: u16, count: u8) -> Self {
        Self {
            item_id,
            item_count: count,
            patch: Some(vec!["custom_name".to_string(), "lore".to_string()]),
        }
    }

    const fn is_empty(&self) -> bool {
        self.item_count == 0
    }
}

/// 比较器容器：9 槽（4 非空 + 5 空）。
fn comparator_container() -> Vec<ModelStack> {
    let mut v = Vec::with_capacity(9);
    for i in 0..9u16 {
        if i % 2 == 0 && i < 8 {
            v.push(ModelStack::filled(100 + i, 3 + (i % 5) as u8));
        } else {
            v.push(ModelStack::empty());
        }
    }
    v
}

/// 漏斗 5 槽（2 非空 + 3 空）。
fn hopper_container() -> Vec<ModelStack> {
    vec![
        ModelStack::filled(200, 16),
        ModelStack::empty(),
        ModelStack::filled(201, 7),
        ModelStack::empty(),
        ModelStack::empty(),
    ]
}

/// 上方箱子 27 槽（3 非空 + 24 空）。
fn chest_container() -> Vec<ModelStack> {
    let mut v = Vec::with_capacity(27);
    for i in 0..27u16 {
        if i % 9 == 1 {
            v.push(ModelStack::filled(300 + i, 12));
        } else {
            v.push(ModelStack::empty());
        }
    }
    v
}

/// 玩家背包 36 槽（8 非空 + 28 空），非空全为同物品。
fn player_container() -> Vec<ModelStack> {
    let mut v = Vec::with_capacity(36);
    for i in 0..36u16 {
        if i % 4 == 0 && i < 32 {
            v.push(ModelStack::filled(400, 5 + (i % 3) as u8));
        } else {
            v.push(ModelStack::empty());
        }
    }
    v
}

/// 比较器输出计算（旧臂：逐槽深克隆）。
fn comparator_old(inv: &[ModelStack]) -> u64 {
    let mut fill_sum = 0.0f32;
    let mut non_empty_count = 0u32;
    for i in 0..inv.len() {
        let stack = inv[i].clone();
        if !stack.is_empty() {
            let max_stack = 64.0f32;
            fill_sum += f32::from(stack.item_count) / max_stack;
            non_empty_count += 1;
        }
    }
    u64::from((fill_sum * 14.0) as u32) + u64::from(non_empty_count)
}

/// 比较器输出计算（新臂：借用读取）。
fn comparator_new(inv: &[ModelStack]) -> u64 {
    let mut fill_sum = 0.0f32;
    let mut non_empty_count = 0u32;
    for i in 0..inv.len() {
        let stack = &inv[i];
        if !stack.is_empty() {
            let max_stack = 64.0f32;
            fill_sum += f32::from(stack.item_count) / max_stack;
            non_empty_count += 1;
        }
    }
    u64::from((fill_sum * 14.0) as u32) + u64::from(non_empty_count)
}

/// 漏斗双段扫描（旧臂：逐槽深克隆；事件无监听零构造）。
fn hopper_old(hopper: &[ModelStack], chest: &[ModelStack]) -> (u64, u64) {
    let mut eject_candidates = 0u64;
    for slot in 0..hopper.len() {
        let item = hopper[slot].clone();
        if item.is_empty() {
            continue;
        }
        eject_candidates += u64::from(item.item_id);
    }
    let mut pickup_candidates = 0u64;
    for i in 0..chest.len() {
        let item = chest[i].clone();
        if !item.is_empty() {
            pickup_candidates += u64::from(item.item_id);
        }
    }
    (eject_candidates, pickup_candidates)
}

/// 漏斗双段扫描（新臂：空槽借用探测，非空也仅借用读取）。
fn hopper_new(hopper: &[ModelStack], chest: &[ModelStack]) -> (u64, u64) {
    let mut eject_candidates = 0u64;
    for slot in 0..hopper.len() {
        let item = &hopper[slot];
        if item.is_empty() {
            continue;
        }
        eject_candidates += u64::from(item.item_id);
    }
    let mut pickup_candidates = 0u64;
    for i in 0..chest.len() {
        let item = &chest[i];
        if !item.is_empty() {
            pickup_candidates += u64::from(item.item_id);
        }
    }
    (eject_candidates, pickup_candidates)
}

/// `count()` 同物品计数（旧臂：逐槽深克隆）。
fn count_old(inv: &[ModelStack], target: u16) -> u64 {
    let mut count = 0u64;
    for i in 0..inv.len() {
        let stack = inv[i].clone();
        if stack.item_id == target {
            count += u64::from(stack.item_count);
        }
    }
    count
}

/// `count()` 同物品计数（新臂：借用累加）。
fn count_new(inv: &[ModelStack], target: u16) -> u64 {
    let mut count = 0u64;
    for i in 0..inv.len() {
        let stack = &inv[i];
        if stack.item_id == target {
            count += u64::from(stack.item_count);
        }
    }
    count
}

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

fn arm_old(
    cmp: &[ModelStack],
    hop: &[ModelStack],
    chest: &[ModelStack],
    bag: &[ModelStack],
) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_ROUNDS {
        fp = fp.wrapping_mul(31).wrapping_add(comparator_old(cmp));
        let (e, p) = hopper_old(hop, chest);
        fp = fp.wrapping_mul(31).wrapping_add(e).wrapping_add(p);
        fp = fp.wrapping_mul(31).wrapping_add(count_old(bag, 400));
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn arm_new(
    cmp: &[ModelStack],
    hop: &[ModelStack],
    chest: &[ModelStack],
    bag: &[ModelStack],
) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_ROUNDS {
        fp = fp.wrapping_mul(31).wrapping_add(comparator_new(cmp));
        let (e, p) = hopper_new(hop, chest);
        fp = fp.wrapping_mul(31).wrapping_add(e).wrapping_add(p);
        fp = fp.wrapping_mul(31).wrapping_add(count_new(bag, 400));
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

    println!("== 轮次 36 双臂基准：容器读取借用化（get_stack 深克隆 → with_stack） ==");
    println!(
        "负载：{N_ROUNDS} 混合轮（比较器 9 槽 + 漏斗 5+27 槽 + count 36 槽；非空槽各带 2 组件）\n"
    );

    let cmp = comparator_container();
    let hop = hopper_container();
    let chest = chest_container();
    let bag = player_container();

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    let old = arm_old(&cmp, &hop, &chest, &bag);
    let new = arm_new(&cmp, &hop, &chest, &bag);
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
    let g2 = old.delta.0 >= N_ROUNDS * ALLOCS_PER_ROUND && new.delta.0 == 0;
    println!(
        "G2 旧臂 ≥ {}（实际 {}）、新臂 == 0（实际 {}）→ {}",
        N_ROUNDS * ALLOCS_PER_ROUND,
        old.delta.0,
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
