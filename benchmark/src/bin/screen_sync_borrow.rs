//! 轮次 31 基准：容器内容同步比较-先行预检（借用读取零克隆）。
//!
//! 对照形态（`ScreenHandler::send_content_updates` /
//! `update_to_client`，轮次 30 收编后的现状 vs 本轮）：
//! - 旧臂（轮次 30 后）：每槽一次 `get_cloned_stack()`（组件补丁
//!   深克隆）+ 引用比较——未变化槽位仍为比较付读取克隆；
//! - 新臂（收编）：`Inventory::with_stack` 借用回调在锁内以引用
//!   与两份跟踪态比较（比较-先行预检），未变化槽位零克隆、
//!   零分配；变化槽位回落完整路径（读取克隆 + 条件内克隆），
//!   次数与旧臂一致。
//!
//! 负载模型与轮次 30 相同（46 槽：12 带补丁、26 裸物品、8 空；
//! `ModelStack.patch` 克隆成本与生产组件补丁 Vec 同构），外加
//! `borrowed: Vec<ModelStack>` 模拟物品栏存储（`with_stack` 的
//! 借用读取直接引用它，`get_cloned_stack` 则克隆）：
//! - 稳态段：10 000 次全屏扫描，零槽位变化；
//! - 扰动段：10 000 次扫描，每次扫描前轮转修改一个槽位
//!   （数量/组件交替），两臂同付变更成本。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 稳态段两臂触发序列指纹一致（空序列固定折叠）；
//! - G2 扰动段两臂触发序列指纹一致；
//! - G3 稳态段新臂零分配（比较-先行：未变化槽位不付任何读取
//!   克隆——带补丁槽位也不付）；
//! - G4 稳态段旧臂分配次数 ≥ 46×扫描数的一半（每扫描至少全部
//!   带补丁槽各付一次读取克隆链，加上裸物品/空槽的 Vec 空克隆
//!   为零——下限取带补丁槽数 × 扫描数 × 每链最小 1 次分配）；
//! - G5 扰动段旧臂分配次数 > 新臂（变化槽位两臂同付，未变化
//!   槽位仅旧臂付读取克隆）。

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

/// 屏幕槽位数（玩家屏幕量级）。
const N_SLOTS: usize = 46;
/// 带组件补丁的槽位数。
const PATCHED_SLOTS: usize = 12;
/// 每段扫描次数。
const N_SCANS: u64 = 10_000;

/// 模型物品堆：`patch` 克隆成本与生产组件补丁 Vec 同构。
#[derive(Clone)]
struct ModelStack {
    count: u8,
    id: u32,
    patch: Vec<(u32, Box<[u8]>)>,
}

impl ModelStack {
    const fn bare(count: u8, id: u32) -> Self {
        Self {
            count,
            id,
            patch: Vec::new(),
        }
    }

    fn with_patch(count: u8, id: u32, components: usize) -> Self {
        Self {
            count,
            id,
            patch: (0..components)
                .map(|c| (c as u32, vec![7u8; 8 + c].into_boxed_slice()))
                .collect(),
        }
    }

    fn are_equal(&self, other: &Self) -> bool {
        self.count == other.count
            && self.id == other.id
            && self.patch.len() == other.patch.len()
            && self
                .patch
                .iter()
                .zip(&other.patch)
                .all(|(a, b)| a.0 == b.0 && a.1 == b.1)
    }
}

/// 模型屏幕：槽位存储 + 两份跟踪态（`tracked_stacks` 与
/// `previous_tracked_stacks` 的比较语义）。
struct ModelScreen {
    slots: Vec<ModelStack>,
    tracked_stacks: Vec<ModelStack>,
    previous_tracked_stacks: Vec<Option<ModelStack>>,
}

impl ModelScreen {
    fn new(slots: Vec<ModelStack>) -> Self {
        let n = slots.len();
        Self {
            slots,
            tracked_stacks: (0..n).map(|_| ModelStack::bare(0, 0)).collect(),
            previous_tracked_stacks: (0..n).map(|_| None).collect(),
        }
    }

    fn fingerprint(slot: usize, stack: &ModelStack) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        h ^= u64::from(slot as u32);
        h = h.wrapping_mul(0x1000_0000_01B3);
        h ^= u64::from(stack.count);
        h = h.wrapping_mul(0x1000_0000_01B3);
        h ^= u64::from(stack.id);
        h = h.wrapping_mul(0x1000_0000_01B3);
        h
    }
}

// ---------------------------------------------------------------------------
// 旧臂（轮次 30 后形态）：每槽读取克隆 + 引用比较
// ---------------------------------------------------------------------------

fn update_tracked_slot(screen: &mut ModelScreen, slot: usize, stack: &ModelStack) {
    if !screen.tracked_stacks[slot].are_equal(stack) {
        screen.tracked_stacks[slot] = stack.clone();
    }
}

fn check_slot_updates(screen: &mut ModelScreen, slot: usize, stack: &ModelStack) {
    let Some(prev) = &screen.previous_tracked_stacks[slot] else {
        screen.previous_tracked_stacks[slot] = Some(stack.clone());
        return;
    };
    if !prev.are_equal(stack) {
        screen.previous_tracked_stacks[slot] = Some(stack.clone());
    }
}

fn scan_clone_read(screen: &mut ModelScreen) -> u64 {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..screen.slots.len() {
        let stack = screen.slots[i].clone();
        let before = screen.tracked_stacks[i].are_equal(&stack);
        update_tracked_slot(screen, i, &stack);
        if !before {
            fp ^= ModelScreen::fingerprint(i, &stack);
        }
        check_slot_updates(screen, i, &stack);
    }
    fp
}

// ---------------------------------------------------------------------------
// 新臂：with_stack 借用比较-先行预检，未变化零克隆
// ---------------------------------------------------------------------------

fn scan_borrow_precheck(screen: &mut ModelScreen) -> u64 {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..screen.slots.len() {
        // 比较-先行预检：两份跟踪态都与槽位当前内容一致时跳过
        //（received_stack 存在且相等；初态/哈希路径判否回落完整
        // 路径——与生产 send_content_updates 预检同构）。
        let unchanged = {
            let stack = &screen.slots[i];
            screen.tracked_stacks[i].are_equal(stack)
                && matches!(&screen.previous_tracked_stacks[i], Some(prev) if prev.are_equal(stack))
        };
        if !unchanged {
            let stack = screen.slots[i].clone();
            let before = screen.tracked_stacks[i].are_equal(&stack);
            update_tracked_slot(screen, i, &stack);
            if !before {
                fp ^= ModelScreen::fingerprint(i, &stack);
            }
            check_slot_updates(screen, i, &stack);
        }
    }
    fp
}

// ---------------------------------------------------------------------------
// 段执行
// ---------------------------------------------------------------------------

/// 段读数：结果指纹 + 非空拍数 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    triggers: u64,
    delta: Delta,
    ns: u128,
}

fn build_slots() -> Vec<ModelStack> {
    let mut slots = Vec::with_capacity(N_SLOTS);
    for i in 0..N_SLOTS {
        let slot = if i < PATCHED_SLOTS {
            ModelStack::with_patch(
                u8::try_from(1 + i % 3).unwrap_or(1),
                100 + i as u32,
                3 + i % 3,
            )
        } else if i % 3 == 0 {
            ModelStack::bare(0, 0)
        } else {
            ModelStack::bare(u8::try_from(8 + i % 32).unwrap_or(8), 500 + i as u32)
        };
        slots.push(slot);
    }
    slots
}

fn perturbation(k: u64, screen: &mut ModelScreen) {
    let slot = (k as usize) % PATCHED_SLOTS;
    let variant = (k / PATCHED_SLOTS as u64) % 4;
    let base = &screen.slots[slot];
    let mut next = base.clone();
    match variant {
        0 => next.count = next.count.wrapping_add(1).max(1),
        1 => next.count = next.count.saturating_sub(1).max(1),
        2 => next.patch.push((99, vec![1u8; 12].into_boxed_slice())),
        _ => {
            next.patch.pop();
            if next.patch.is_empty() {
                next.patch.push((7, vec![3u8; 9].into_boxed_slice()));
            }
        }
    }
    screen.slots[slot] = next;
}

fn steady_segment(scan: fn(&mut ModelScreen) -> u64) -> Report {
    let mut screen = ModelScreen::new(build_slots());
    black_box(scan(&mut screen));
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_SCANS {
        fp ^= scan(&mut screen);
    }
    let delta = delta_since(start);
    Report {
        fp,
        triggers: 0,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn perturbed_segment(scan: fn(&mut ModelScreen) -> u64) -> Report {
    let mut screen = ModelScreen::new(build_slots());
    black_box(scan(&mut screen));
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let mut triggers = 0u64;
    let t0 = Instant::now();
    let start = snapshot();
    for k in 0..N_SCANS {
        perturbation(k, &mut screen);
        let scan_fp = scan(&mut screen);
        if scan_fp != 0xcbf2_9ce4_8422_2325u64 {
            triggers += 1;
        }
        fp ^= scan_fp;
    }
    let delta = delta_since(start);
    Report {
        fp,
        triggers,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn print_segment(title: &str, old: &Report, new: &Report) {
    println!(
        "{title} 段 旧 {:>8} allocs / {:>10} B / {:>10} ns",
        old.delta.0, old.delta.1, old.ns
    );
    println!(
        "{title} 段 新 {:>8} allocs / {:>10} B / {:>10} ns",
        new.delta.0, new.delta.1, new.ns
    );
}

fn main() {
    let mut gates_pass = true;

    println!("== 轮次 31 双臂基准：容器同步比较-先行预检（借用零克隆）==");
    println!(
        "负载：{N_SLOTS} 槽（{PATCHED_SLOTS} 带补丁），每段 {N_SCANS} 次扫描（稳态零变化 / 扰动每拍一变）\n"
    );

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    // 段 1：稳态（零变化）
    let old = steady_segment(scan_clone_read);
    let new = steady_segment(scan_borrow_precheck);
    print_segment("稳态", &old, &new);
    let g1 = old.fp == new.fp;
    println!(
        "G1 稳态段触发序列指纹一致（{:#x} vs {:#x}）→ {}",
        old.fp,
        new.fp,
        verdict(g1)
    );
    gates_pass &= g1;
    let g3 = new.delta.0 == 0;
    println!(
        "G3 稳态段新臂零分配（实际 {}）→ {}",
        new.delta.0,
        verdict(g3)
    );
    gates_pass &= g3;
    let g4 = old.delta.0 >= N_SCANS * PATCHED_SLOTS as u64;
    println!(
        "G4 稳态段旧臂分配 ≥ {}（带补丁槽每扫描一链；实际 {}）→ {}",
        N_SCANS * PATCHED_SLOTS as u64,
        old.delta.0,
        verdict(g4)
    );
    gates_pass &= g4;

    // 段 2：扰动（每拍一变）
    let old = perturbed_segment(scan_clone_read);
    let new = perturbed_segment(scan_borrow_precheck);
    print_segment("扰动", &old, &new);
    let g2 = old.fp == new.fp;
    println!(
        "G2 扰动段触发序列指纹一致（{:#x} vs {:#x}）→ {}",
        old.fp,
        new.fp,
        verdict(g2)
    );
    gates_pass &= g2;
    let g5 = old.triggers > 0 && new.triggers > 0 && old.delta.0 > new.delta.0;
    println!(
        "G5 扰动段触发非空（旧 {} / 新 {} 拍）且旧臂分配 > 新臂（{} > {}）→ {}",
        old.triggers,
        new.triggers,
        old.delta.0,
        new.delta.0,
        verdict(g5)
    );
    gates_pass &= g5;

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
