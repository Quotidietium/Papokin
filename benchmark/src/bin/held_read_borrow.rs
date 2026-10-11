//! 轮次 37 基准：手持物品只读访问的借用化。
//!
//! 对照形态（本轮全仓 86 个 `held_item` / `off_hand_item` /
//! `get_stack_in_hand` 调用点甄别后收编约 30 处纯读白付）：
//! - 旧臂（现状）：`held_item()` / `get_stack_in_hand()` 每次
//!   调用都克隆整栈——带组件补丁的栈为 1 次 Vec 分配 + 每组件
//!   一次 dyn 克隆（模型记 1 Vec + 2 String）；挖掘速度、AI
//!   双手探测、统计读取等只读场景全部白付；
//! - 新臂（收编）：`with_held_item` / `with_stack_in_hand` 锁内
//!   借用回调，零克隆零分配（回调内仅调用 `ItemStack` 自身方法，
//!   不重入物品栏）。
//!
//! 负载模型（「玩家持续活动」混合轮：选中方块循环挖掘 + 周围
//! AI 每 tick 探测 + 使用统计 + 每 4 轮一次格挡损伤事件门）：
//! - 挖掘路径 ×3：`can_harvest` 收获判定 / `get_mining_speed` 挖掘
//!   速度 / EFFICIENCY 附魔等级（3 次读）；
//! - AI 探测 ×2：beg / tempt 目标双手 `is_empty` + 物品 id 比对
//!   （2 次读）；
//! - 统计读取 ×1：Used 分类 `item_id`（1 次读）；
//! - 每 4 轮：实体物品损伤事件（旧臂无条件构造含栈克隆；新臂
//!   订阅者先行门无监听零构造）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 两臂语义指纹一致（收获/速度/等级/统计/事件未取消折叠）；
//! - G2 旧臂分配 ≥ 轮数 × 18（每轮 6 读 × 3 分配）+ 每 4 轮
//!   事件 3 分配，新臂 == 0；
//! - G3 新臂耗时 ≤ 旧臂；
//! - G4 旧臂毛字节 > 新臂（>0 vs 0）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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
/// 每 6 次手持读取附一次格挡损伤事件（盾牌节奏）。
const DAMAGE_EVENT_EVERY: u64 = 4;
/// 每轮手持读取次数（挖掘 3 + AI 2 + 统计 1）。
const READS_PER_ROUND: u64 = 6;
/// 每次整栈克隆的分配数（1 Vec + 2 String 组件克隆）。
const ALLOCS_PER_CLONE: u64 = 3;

/// 模型物品栈：物品 id + 数量 + 组件补丁。
/// 补丁镜像真实 `patch: Vec<(DataComponent, ...)>` 的分配面：
/// 克隆 = 1 次 Vec 缓冲 + 每条目一次堆分配。
#[derive(Clone)]
struct ModelStack {
    item_id: u32,
    count: u8,
    patch: Option<Vec<String>>,
}

impl ModelStack {
    const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// 镜像 `get_mining_speed`：工具基础速度 + 效率附魔加成。
    fn mining_speed(&self) -> f64 {
        let mut speed = if self.item_id >= 2 { 8.0 } else { 1.0 };
        if let Some(patch) = &self.patch {
            for entry in patch {
                if let Some(level) = enchant_value(entry, "efficiency") {
                    speed += f64::from(level) * f64::from(level) + 1.0;
                }
            }
        }
        speed
    }

    /// 镜像 `get_enchantment_level`：按名取等级。
    fn enchant_level(&self, name: &str) -> u32 {
        let mut level = 0;
        if let Some(patch) = &self.patch {
            for entry in patch {
                if let Some(v) = enchant_value(entry, name) {
                    level = v;
                }
            }
        }
        level
    }
}

/// 解析 "name=level" 形式的补丁条目。
fn enchant_value(entry: &str, name: &str) -> Option<u32> {
    let (key, value) = entry.split_once('=')?;
    if key == name {
        value.parse().ok()
    } else {
        None
    }
}

/// 模型事件：仅保留栈分配面（数值字段对分配无贡献）。
#[allow(dead_code)] // 分配面模型：字段仅构造，经 black_box 保活
struct ModelDamageEvent {
    stack: ModelStack,
    damage: i32,
}

/// 模型玩家物品栏：主物品栏读锁 + 选中槽（镜像 `PlayerInventory`
/// 的 `main_inventory: RwLock<Vec<..>>` + `selected_slot` 快捷栏结构）。
struct ModelInventory {
    main: RwLock<Vec<ModelStack>>,
    selected: AtomicUsize,
}

impl ModelInventory {
    fn new() -> Self {
        // 快捷栏 9 槽：选中槽持镐（效率 3 + 耐久 2 双组件），
        // 其余为空槽
        let mut main = vec![
            ModelStack {
                item_id: 0,
                count: 0,
                patch: None,
            };
            9
        ];
        main[3] = ModelStack {
            item_id: 7,
            count: 1,
            patch: Some(vec!["efficiency=3".to_string(), "unbreaking=2".to_string()]),
        };
        Self {
            main: RwLock::new(main),
            selected: AtomicUsize::new(3),
        }
    }

    /// 旧臂镜像：`held_item()`——锁内读 + 整栈克隆。
    fn held_item(&self) -> ModelStack {
        let inv = self
            .main
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selected = self.selected.load(Ordering::Relaxed);
        inv[selected].clone()
    }

    /// 新臂镜像：`with_held_item`——锁内借用回调，零克隆。
    fn with_held_item(&self, f: &mut dyn FnMut(&ModelStack)) {
        let inv = self
            .main
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selected = self.selected.load(Ordering::Relaxed);
        f(&inv[selected]);
    }
}

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

/// 一轮玩家活动负载的读取面（旧臂：每读一克隆）。
fn round_old(inv: &ModelInventory, round: u64, fp: u64) -> u64 {
    let mut fp = fp;
    // 挖掘路径 ×3（收获判定 / 挖掘速度 / 效率等级）
    for _ in 0..3 {
        let held = inv.held_item();
        let harvest = !held.is_empty() && held.item_id >= 2;
        let speed = held.mining_speed();
        let level = held.enchant_level("efficiency");
        fp = fp
            .wrapping_mul(31)
            .wrapping_add(u64::from(harvest) + speed.to_bits() + u64::from(level));
        black_box(&held);
    }
    // AI 探测 ×2（beg / tempt：is_empty + 物品 id 比对）
    for _ in 0..2 {
        let held = inv.held_item();
        let interesting = !held.is_empty() && held.item_id == 7;
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(interesting));
        black_box(&held);
    }
    // 统计读取 ×1（Used 分类 item_id）
    let held = inv.held_item();
    fp = fp.wrapping_mul(31).wrapping_add(u64::from(held.item_id));
    black_box(&held);
    // 每 4 轮：实体物品损伤事件（旧臂无条件构造，含栈克隆）
    if round.is_multiple_of(DAMAGE_EVENT_EVERY) {
        let event = ModelDamageEvent {
            stack: inv.held_item(),
            damage: 3,
        };
        black_box(&event);
        fp = fp.wrapping_mul(31); // cancelled=false 语义
    }
    fp
}

/// 一轮玩家活动负载的读取面（新臂：锁内借用，零克隆）。
fn round_new(inv: &ModelInventory, round: u64, fp: u64) -> u64 {
    let mut fp = fp;
    // 挖掘路径 ×3
    for _ in 0..3 {
        inv.with_held_item(&mut |held| {
            let harvest = !held.is_empty() && held.item_id >= 2;
            let speed = held.mining_speed();
            let level = held.enchant_level("efficiency");
            fp = fp
                .wrapping_mul(31)
                .wrapping_add(u64::from(harvest) + speed.to_bits() + u64::from(level));
        });
    }
    // AI 探测 ×2
    for _ in 0..2 {
        inv.with_held_item(&mut |held| {
            let interesting = !held.is_empty() && held.item_id == 7;
            fp = fp.wrapping_mul(31).wrapping_add(u64::from(interesting));
        });
    }
    // 统计读取 ×1
    inv.with_held_item(&mut |held| {
        fp = fp.wrapping_mul(31).wrapping_add(u64::from(held.item_id));
    });
    // 每 4 轮：损伤事件订阅者先行门（无监听零构造，cancelled
    // 语义保持 false，与旧臂 fire 无处理器短路同源）
    if round.is_multiple_of(DAMAGE_EVENT_EVERY) {
        let has_handlers = false;
        if has_handlers {
            inv.with_held_item(&mut |held| {
                let event = ModelDamageEvent {
                    stack: held.clone(),
                    damage: 3,
                };
                black_box(&event);
            });
        }
        fp = fp.wrapping_mul(31);
    }
    fp
}

fn arm(round: fn(&ModelInventory, u64, u64) -> u64, inv: &ModelInventory) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for i in 0..N_ROUNDS {
        fp = round(inv, i, fp);
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

    println!("== 轮次 37 双臂基准：手持物品只读访问借用化 ==");
    println!(
        "负载：{N_ROUNDS} 混合轮（挖掘 3 读 + AI 2 读 + 统计 1 读；每 {DAMAGE_EVENT_EVERY} 轮附损伤事件门），选中方块循环\n"
    );

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    let inv = ModelInventory::new();
    let old = arm(round_old, &inv);
    let new = arm(round_new, &inv);
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
    // 每轮 6 读 × 3 分配 + 每 4 轮事件 1 克隆 × 3 分配
    let old_floor = N_ROUNDS * READS_PER_ROUND * ALLOCS_PER_CLONE
        + (N_ROUNDS / DAMAGE_EVENT_EVERY) * ALLOCS_PER_CLONE;
    let g2 = old.delta.0 >= old_floor && new.delta.0 == 0;
    println!(
        "G2 旧臂 ≥ {}（实际 {}）、新臂 == 0（实际 {}）→ {}",
        old_floor,
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
