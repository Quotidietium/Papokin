//! 轮次 33 基准：玩家移动路径逐 tick 分配面（复合两点）。
//!
//! 对照形态（`JavaClient` 移动包处理路径 `handle_position` /
//! `handle_position_rotation`）：
//! - 旧臂（现状）：
//!   - 统计递增：`Player::increment_stat` 在检查订阅者之前就查表
//!     取玩家并 `format!("{category:?}:{stat}")` 构造事件字符串
//!     ——移动路径每包（cm>0）都走这里，无插件订阅时 String
//!     分配全部白付（`fire_blocking` 内部短路救不回）；
//!   - 跨方块群系刷新：`set_pos` 每次跨方块都
//!     `current_biome.store(Arc::new(biome))`——注册表
//!     `&'static Biome` 被 Arc 包装纯花一次堆分配，同群系内
//!     的连续跨界占绝对多数；
//! - 新臂（收编）：
//!   - 统计递增：`has_handlers::<PlayerStatisticIncrementEvent>()`
//!     先行，无监听时零构造零分配；
//!   - 跨方块群系刷新：`from_id` 为 const 静态表（同群系恒同址），
//!     指针去重门，群系未变时跳过 Arc 包装。
//!
//! 负载模型（持续移动玩家：每包 cm>0；真 `ArcSwap<&'static Biome>` +
//! 真 `Biome::from_id` 注册表静态表）：
//! - 段 1（统计递增，无订阅者模型）：600 000 个移动包，每包一次
//!   `increment_stat` 等价调用；
//! - 段 2（跨方块群系刷新）：300 000 次跨方块，每 64 次跨界才
//!   变更一次群系（同群系连续跨界模型）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 段 1 两臂统计指纹一致（无订阅者时 `final_amount` == amount，
//!   事件不可观测）；
//! - G2 段 1 旧臂分配 ≥ 包数（每包一次 String）、新臂 == 0；
//! - G3 段 1 新臂耗时 ≤ 旧臂；
//! - G4 段 2 两臂群系指纹逐步一致（去重门不得留下陈旧群系——
//!   `current_biome` 必须始终反映最新方块位置的群系）；
//! - G5 段 2 旧臂分配 ≥ 跨界数（每次一次 Arc）、新臂 ≤
//!   变更数 + 1（去重门只放行真实变更）；
//! - G6 段 2 新臂耗时 ≤ 旧臂。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use arc_swap::ArcSwap;
use papokin_data::biome::Biome;

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

/// 段 1：移动包数（每包 cm>0，模型化持续行走玩家）。
const N_PACKETS: u64 = 600_000;
/// 段 2：跨方块次数。
const N_CROSSINGS: u64 = 300_000;
/// 同一群系内的连续跨界数（群系变更节奏）。
const BIOME_HOLD: u64 = 64;
/// 轮换的注册表群系数（id 0..11，均为 const 静态表项）。
const N_BIOMES: u64 = 12;

/// 镜像 `StatisticCategory` 的 Debug 格式化面（生产格式化
/// "{category:?}:{stat}" 的分配来自 format 机器本身，枚举变体
/// 名长度同量级）。
#[derive(Debug)]
enum ModelStatCategory {
    Custom,
    Mined,
    Crafted,
    Broken,
    Used,
}

const CATEGORIES: [ModelStatCategory; 5] = [
    ModelStatCategory::Custom,
    ModelStatCategory::Mined,
    ModelStatCategory::Crafted,
    ModelStatCategory::Broken,
    ModelStatCategory::Used,
];

/// 模型事件（复刻 `PlayerStatisticIncrementEvent` 的分配面：
/// `statistic_id` String；player 为 Arc 计数克隆，无堆分配）。
struct ModelStatEvent {
    statistic_id: String,
    amount: i32,
    cancelled: bool,
}

fn biome_at(crossing: u64) -> &'static Biome {
    Biome::from_id(u8::try_from((crossing / BIOME_HOLD) % N_BIOMES).unwrap_or(0))
        .unwrap_or(&Biome::THE_VOID)
}

// ---------------------------------------------------------------------------
// 段 1：统计递增路径（无订阅者模型）
// ---------------------------------------------------------------------------

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

/// 旧臂：构造事件（含 format! String）后才在 fire 内部发现无处理器。
fn stat_old() -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for i in 0..N_PACKETS {
        let category = &CATEGORIES[(i % CATEGORIES.len() as u64) as usize];
        let stat = 70 + (i % 4) as i32;
        let amount = 3 + (i % 5) as i32;
        let mut event = ModelStatEvent {
            statistic_id: format!("{category:?}:{stat}"),
            amount,
            cancelled: false,
        };
        // fire 镜像：无处理器早退（black_box 防止整体被优化掉）
        black_box(&mut event);
        if event.cancelled {
            continue;
        }
        let final_amount = event.amount;
        fp = fp.wrapping_mul(31).wrapping_add(final_amount as u64);
        black_box(&event.statistic_id);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

/// 新臂：订阅者先行（无监听为假），构造路径不进入。
fn stat_new() -> Report {
    let has_handlers = false;
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for i in 0..N_PACKETS {
        let category = &CATEGORIES[(i % CATEGORIES.len() as u64) as usize];
        let stat = 70 + (i % 4) as i32;
        let amount = 3 + (i % 5) as i32;
        let final_amount = if has_handlers {
            let mut event = ModelStatEvent {
                statistic_id: format!("{category:?}:{stat}"),
                amount,
                cancelled: false,
            };
            black_box(&mut event);
            if event.cancelled {
                continue;
            }
            event.amount
        } else {
            amount
        };
        fp = fp.wrapping_mul(31).wrapping_add(final_amount as u64);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

// ---------------------------------------------------------------------------
// 段 2：跨方块群系刷新（真 ArcSwap<&'static Biome> + 真注册表）
// ---------------------------------------------------------------------------

/// 旧臂：每次跨方块都 `store(Arc::new(biome))`。
fn biome_old(swap: &ArcSwap<&'static Biome>) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for crossing in 0..N_CROSSINGS {
        let biome = biome_at(crossing);
        swap.store(Arc::new(biome));
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(swap.load().id));
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

/// 新臂：指针去重门——注册表同群系恒同址，未变更时跳过 Arc 包装。
fn biome_new(swap: &ArcSwap<&'static Biome>) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for crossing in 0..N_CROSSINGS {
        let biome = biome_at(crossing);
        if !std::ptr::eq(**swap.load(), biome) {
            swap.store(Arc::new(biome));
        }
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(swap.load().id));
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

    println!("== 轮次 33 双臂基准：玩家移动路径逐 tick 分配面 ==");
    println!(
        "负载：段1 {N_PACKETS} 移动包（无订阅者）；段2 {N_CROSSINGS} 跨方块（每 {BIOME_HOLD} 次变一群系）\n"
    );

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    // 段 1：统计递增（无订阅者）
    let old = stat_old();
    let new = stat_new();
    println!(
        "统计段 旧 {:>8} allocs / {:>9} B / {:>10} ns",
        old.delta.0, old.delta.1, old.ns
    );
    println!(
        "统计段 新 {:>8} allocs / {:>9} B / {:>10} ns",
        new.delta.0, new.delta.1, new.ns
    );
    let g1 = old.fp == new.fp;
    println!(
        "G1 统计段两臂指纹一致（{:#x} vs {:#x}）→ {}",
        old.fp,
        new.fp,
        verdict(g1)
    );
    gates_pass &= g1;
    let g2 = old.delta.0 >= N_PACKETS && new.delta.0 == 0;
    println!(
        "G2 统计段旧臂 ≥ {N_PACKETS}（实际 {}）、新臂 == 0（实际 {}）→ {}",
        old.delta.0,
        new.delta.0,
        verdict(g2)
    );
    gates_pass &= g2;
    let g3 = new.ns <= old.ns;
    println!(
        "G3 统计段新臂耗时 ≤ 旧臂（{} ns ≤ {} ns）→ {}",
        new.ns,
        old.ns,
        verdict(g3)
    );
    gates_pass &= g3;

    // 段 2：跨方块群系刷新（两臂独立实例，避免互染）
    let swap_old = ArcSwap::new(Arc::new(biome_at(0)));
    let swap_new = ArcSwap::new(Arc::new(biome_at(0)));
    let old_b = biome_old(&swap_old);
    let new_b = biome_new(&swap_new);
    println!(
        "群系段 旧 {:>8} allocs / {:>9} B / {:>10} ns",
        old_b.delta.0, old_b.delta.1, old_b.ns
    );
    println!(
        "群系段 新 {:>8} allocs / {:>9} B / {:>10} ns",
        new_b.delta.0, new_b.delta.1, new_b.ns
    );
    let g4 = old_b.fp == new_b.fp;
    println!(
        "G4 群系段两臂逐步指纹一致（{:#x} vs {:#x}，去重不留陈旧群系）→ {}",
        old_b.fp,
        new_b.fp,
        verdict(g4)
    );
    gates_pass &= g4;
    let changes = N_CROSSINGS.div_ceil(BIOME_HOLD);
    let g5 = old_b.delta.0 >= N_CROSSINGS && new_b.delta.0 <= changes + 1;
    println!(
        "G5 群系段旧臂 ≥ {N_CROSSINGS}（实际 {}）、新臂 ≤ {}（实际 {}）→ {}",
        old_b.delta.0,
        changes + 1,
        new_b.delta.0,
        verdict(g5)
    );
    gates_pass &= g5;
    let g6 = new_b.ns <= old_b.ns;
    println!(
        "G6 群系段新臂耗时 ≤ 旧臂（{} ns ≤ {} ns）→ {}",
        new_b.ns,
        old_b.ns,
        verdict(g6)
    );
    gates_pass &= g6;

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
